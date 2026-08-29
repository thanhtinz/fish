//! Per-game patches, written as data (§19).
//!
//! Everything else in this crate is general: it works on any JAR because it only does things that
//! are true of every JAR. Some of the work is not like that. Making a game use a new glyph sheet
//! means knowing that *this* game reads `/font.png`, that *this* class holds the number of
//! columns, that *this* string lists the characters in sheet order. None of that can be inferred,
//! and a tool that guessed it would corrupt games.
//!
//! So it is written down instead. A rule says what it expects to find and what it would change,
//! the engine checks the expectations against the actual archive, and refuses when they do not
//! hold. A rule carried over from a different version of a game does not silently patch the wrong
//! constant; it reports that the game does not look like what the rule was written for.
//!
//! What a rule may do is deliberately narrow: replace a resource, and change constants in the
//! pool. Both are things this crate already does safely and has JVM-verified. A rule cannot add
//! bytecode, and so cannot make a class fail verification.

use crate::classfile::ClassFile;
use crate::jar::{sha256_hex, Archive};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Something a rule requires to be true of the game before it will touch it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Condition {
    /// The archive has this entry.
    EntryExists { entry: String },
    /// The entry is byte-for-byte what the rule was written against.
    ///
    /// The strictest condition there is, and the right one for a rule that replaces a file: it
    /// says "this is the image I measured", so a game whose artwork was updated is refused rather
    /// than patched from a stale measurement.
    EntrySha256 { entry: String, sha256: String },
    /// This class holds a `CONSTANT_Integer` of this value.
    IntConstant { class: String, value: i32 },
    /// This class holds this string literal.
    StringConstant { class: String, text: String },
    /// A file the rule needs exists in the project directory.
    ProjectFile { path: String },
}

/// Something a rule does.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    /// Replaces an archive entry with a file from the project directory.
    ReplaceEntry { entry: String, from: String },
    /// Changes every integer constant of one value in one class.
    ///
    /// Scoped to a class and to an exact previous value, because "change the 16 to 22" applied
    /// across a whole game changes sixteens that had nothing to do with the font.
    SetIntConstant { class: String, from: i32, to: i32 },
    /// Changes a string literal in one class.
    ///
    /// This is how a font swap usually lands: a game that draws from a sheet often keeps the
    /// characters, in sheet order, as one string.
    SetStringConstant {
        class: String,
        from: String,
        to: String,
    },
}

impl Action {
    /// The class this action touches, if it touches one.
    fn class(&self) -> Option<&str> {
        match self {
            Action::ReplaceEntry { .. } => None,
            Action::SetIntConstant { class, .. } | Action::SetStringConstant { class, .. } => {
                Some(class)
            }
        }
    }
}

/// One named patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    /// What this rule is for, in a sentence, for the person deciding whether to enable it.
    #[serde(default)]
    pub description: String,
    /// Off unless somebody turned it on. A rule is a change to how a game behaves.
    #[serde(default)]
    pub enabled: bool,
    /// What must be true before it will run.
    #[serde(default)]
    pub when: Vec<Condition>,
    #[serde(default)]
    pub then: Vec<Action>,
}

impl Rule {
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Rule {
            id: id.into(),
            description: description.into(),
            enabled: false,
            when: Vec::new(),
            then: Vec::new(),
        }
    }
}

/// What one rule would do to this game, or why it cannot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulePlan {
    pub id: String,
    pub description: String,
    pub enabled: bool,
    /// Conditions that do not hold. Empty means the rule fits this game.
    pub unmet: Vec<String>,
    /// What it would change, one line each, so a person can read the patch before running it.
    pub effects: Vec<String>,
}

impl RulePlan {
    pub fn ready(&self) -> bool {
        self.enabled && self.unmet.is_empty() && !self.effects.is_empty()
    }
}

/// What actually happened.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub rules: Vec<String>,
    pub entries_replaced: usize,
    pub constants_changed: usize,
}

/// Checks every rule against the archive without changing anything.
///
/// Nothing here is a judgement about whether a rule is a good idea - only about whether the game
/// looks like the game it was written for.
pub fn plan(rules: &[Rule], archive: &Archive, root: &Path) -> Result<Vec<RulePlan>> {
    let mut plans = Vec::new();
    for rule in rules {
        let mut unmet = Vec::new();
        for condition in &rule.when {
            if let Some(reason) = unmet_reason(condition, archive, root)? {
                unmet.push(reason);
            }
        }

        let mut effects = Vec::new();
        for action in &rule.then {
            effects.extend(describe(action, archive, root)?);
        }
        plans.push(RulePlan {
            id: rule.id.clone(),
            description: rule.description.clone(),
            enabled: rule.enabled,
            unmet,
            effects,
        });
    }
    Ok(plans)
}

/// Applies every enabled rule whose conditions hold.
///
/// A rule that does not fit is skipped, not an error: a project can hold rules for several
/// versions of a game, and the point of the conditions is to pick out the one that matches.
pub fn apply(rules: &[Rule], archive: &mut Archive, root: &Path) -> Result<Applied> {
    let mut applied = Applied::default();

    for rule in rules {
        if !rule.enabled {
            continue;
        }
        let mut fits = true;
        for condition in &rule.when {
            if unmet_reason(condition, archive, root)?.is_some() {
                fits = false;
                break;
            }
        }
        if !fits {
            continue;
        }

        // Class actions are grouped so a class holding two changes is parsed and written once.
        let mut touched = false;
        for action in &rule.then {
            if let Action::ReplaceEntry { entry, from } = action {
                let path = root.join(from);
                let data = std::fs::read(&path).map_err(|e| crate::Error::InvalidProject {
                    path: path.clone(),
                    reason: format!("rule {} needs this file: {e}", rule.id),
                })?;
                if archive.replace(entry, data) {
                    applied.entries_replaced += 1;
                    touched = true;
                }
            }
        }

        let classes: Vec<String> = rule
            .then
            .iter()
            .filter_map(|a| a.class().map(|c| c.to_string()))
            .collect();
        for class in dedup(classes) {
            let Some(entry) = archive.get(&class) else {
                continue;
            };
            let mut file = ClassFile::parse(&entry.data)?;
            let mut changed = 0usize;

            for action in &rule.then {
                match action {
                    Action::SetIntConstant {
                        class: c, from, to, ..
                    } if *c == class => {
                        let targets: Vec<u16> = file
                            .integers()
                            .into_iter()
                            .filter(|(_, v)| v == from)
                            .map(|(i, _)| i)
                            .collect();
                        for index in targets {
                            file.set_integer(index, *to)?;
                            changed += 1;
                        }
                    }
                    Action::SetStringConstant {
                        class: c, from, to, ..
                    } if *c == class => {
                        let targets: Vec<u16> = file
                            .string_literals()
                            .into_iter()
                            .filter(|l| l.decoded.as_deref() == Some(from.as_str()))
                            .map(|l| l.utf8_index)
                            .collect();
                        for index in targets {
                            file.set_utf8_text(index, to)?;
                            changed += 1;
                        }
                    }
                    _ => {}
                }
            }

            if changed > 0 {
                archive.replace(&class, file.write()?);
                applied.constants_changed += changed;
                touched = true;
            }
        }

        if touched {
            applied.rules.push(rule.id.clone());
        }
    }
    Ok(applied)
}

fn dedup(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names.dedup();
    names
}

/// Why a condition does not hold, or `None` when it does.
fn unmet_reason(condition: &Condition, archive: &Archive, root: &Path) -> Result<Option<String>> {
    Ok(match condition {
        Condition::EntryExists { entry } => match archive.get(entry) {
            Some(_) => None,
            None => Some(format!("the game has no {entry}")),
        },
        Condition::EntrySha256 { entry, sha256 } => {
            match archive.get(entry) {
                None => Some(format!("the game has no {entry}")),
                Some(found) => {
                    let actual = sha256_hex(&found.data);
                    (actual != *sha256).then(|| {
                    format!("{entry} is not the file this rule was written against (it is {actual})")
                })
                }
            }
        }
        Condition::IntConstant { class, value } => match read_class(archive, class)? {
            None => Some(format!("the game has no {class}")),
            Some(file) => (!file.integers().iter().any(|(_, v)| v == value))
                .then(|| format!("{class} does not hold the number {value}")),
        },
        Condition::StringConstant { class, text } => match read_class(archive, class)? {
            None => Some(format!("the game has no {class}")),
            Some(file) => (!file
                .string_literals()
                .iter()
                .any(|l| l.decoded.as_deref() == Some(text.as_str())))
            .then(|| format!("{class} does not hold the text {text:?}")),
        },
        Condition::ProjectFile { path } => {
            let full = root.join(path);
            (!full.exists()).then(|| format!("the project has no {path} yet"))
        }
    })
}

fn read_class(archive: &Archive, class: &str) -> Result<Option<ClassFile>> {
    match archive.get(class) {
        Some(entry) => Ok(Some(ClassFile::parse(&entry.data)?)),
        None => Ok(None),
    }
}

/// What an action would do, said in numbers taken from this archive rather than from the rule.
///
/// A rule claiming to change a constant that is not there produces no line here, and a plan with
/// no effects is not ready - which is the difference between a patch and a wish.
fn describe(action: &Action, archive: &Archive, root: &Path) -> Result<Vec<String>> {
    Ok(match action {
        Action::ReplaceEntry { entry, from } => {
            let source = root.join(from);
            match (archive.get(entry), source.exists()) {
                (Some(found), true) => {
                    let size = std::fs::metadata(&source).map(|m| m.len()).unwrap_or(0);
                    vec![format!(
                        "replace {entry} ({} bytes) with {from} ({size} bytes)",
                        found.data.len()
                    )]
                }
                _ => vec![],
            }
        }
        Action::SetIntConstant { class, from, to } => match read_class(archive, class)? {
            None => vec![],
            Some(file) => {
                let count = file.integers().iter().filter(|(_, v)| v == from).count();
                if count == 0 {
                    vec![]
                } else {
                    vec![format!("in {class}, change {count} × {from} to {to}")]
                }
            }
        },
        Action::SetStringConstant { class, from, to } => match read_class(archive, class)? {
            None => vec![],
            Some(file) => {
                let count = file
                    .string_literals()
                    .iter()
                    .filter(|l| l.decoded.as_deref() == Some(from.as_str()))
                    .count();
                if count == 0 {
                    vec![]
                } else {
                    vec![format!("in {class}, change {count} × {from:?} to {to:?}")]
                }
            }
        },
    })
}

/// Where a project keeps its rules.
pub fn path(root: &Path) -> PathBuf {
    root.join("rules").join("rules.json")
}

pub fn load(root: &Path) -> Result<Vec<Rule>> {
    let file = path(root);
    if !file.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&file)?;
    serde_json::from_str(&text).map_err(|e| crate::Error::InvalidProject {
        path: file,
        reason: format!("the rules could not be read: {e}"),
    })
}

pub fn save(root: &Path, rules: &[Rule]) -> Result<()> {
    let file = path(root);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&file, serde_json::to_string_pretty(rules)?)?;
    Ok(())
}
