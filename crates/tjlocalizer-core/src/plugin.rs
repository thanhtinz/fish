//! Adapters for one game, or one engine, written as data (§20, §32).
//!
//! The core knows formats, never games. That rule is what makes it work on a game nobody here has
//! seen, and it is also its limit: a game that keeps its text in `data/lang/en.txt` in a shape no
//! detector recognises is invisible to a tool that will not be told about it. Rules (§19) close
//! part of that gap - they patch a named game - but they run at build time and cannot say "this
//! file is a resource" or "this archive is that engine".
//!
//! A plugin says those things. It is a JSON file naming what to look for and what to conclude:
//! capabilities to report when an archive matches, files to treat as resources of a format this
//! build already reads and writes, glyph sheets to suggest, rules to offer, dictionary entries to
//! add. Everything it contributes goes through machinery that already exists; the plugin supplies
//! the game-specific knowledge, and nothing else.
//!
//! **A plugin is data, and only data.** No code is loaded, nothing is executed, and there is no
//! escape hatch that would let a plugin do something this build cannot already do to any archive.
//! That is a deliberate refusal rather than an unfinished feature: the whole point of §29 is that
//! a JAR downloaded from a forum is untrusted input, and a plugin arrives by the same route as
//! the JAR - somebody posts it. A plugin format that could run code would make "open this game"
//! mean "run this stranger's program", and every guarantee in this crate would be worth nothing.
//!
//! What a plugin therefore cannot do: read a format no reader here owns, write bytecode, reach
//! the network, or touch a file outside the project. A game needing any of those needs a change
//! to this crate, and the plugin file is the wrong place to hide it.

use crate::detect::Capability;
use crate::jar::{Archive, Manifest};
use crate::resource::Format;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Something a plugin looks for in an archive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Match {
    /// The archive has this entry, by exact name.
    EntryExists { entry: String },
    /// The archive has an entry whose name matches this pattern, where `*` stands for any run of
    /// characters and `?` for one.
    EntryMatches { pattern: String },
    /// The archive has at least this many entries matching the pattern.
    ///
    /// An engine is recognised by a shape rather than a file: one `.assets` beside a game is a
    /// coincidence, forty of them is Unity.
    EntryCount { pattern: String, at_least: usize },
    /// The JAR manifest has this attribute, optionally containing this text.
    ManifestAttribute {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        contains: Option<String>,
    },
}

impl Match {
    fn holds(&self, archive: &Archive, manifest: Option<&Manifest>) -> bool {
        match self {
            Match::EntryExists { entry } => archive.get(entry).is_some(),
            Match::EntryMatches { pattern } => {
                archive.entries().iter().any(|e| glob(pattern, &e.name))
            }
            Match::EntryCount { pattern, at_least } => {
                archive
                    .entries()
                    .iter()
                    .filter(|e| glob(pattern, &e.name))
                    .count()
                    >= *at_least
            }
            Match::ManifestAttribute { key, contains } => match manifest.and_then(|m| m.get(key)) {
                Some(value) => contains
                    .as_ref()
                    .map(|want| value.contains(want))
                    .unwrap_or(true),
                None => false,
            },
        }
    }

    /// What this match found, for the evidence trail a capability carries (§6).
    fn evidence(&self) -> String {
        match self {
            Match::EntryExists { entry } => format!("the archive has {entry}"),
            Match::EntryMatches { pattern } => format!("an entry matches {pattern}"),
            Match::EntryCount { pattern, at_least } => {
                format!("at least {at_least} entries match {pattern}")
            }
            Match::ManifestAttribute {
                key,
                contains: Some(text),
            } => format!("the manifest's {key} contains {text}"),
            Match::ManifestAttribute { key, .. } => format!("the manifest has {key}"),
        }
    }
}

/// A capability a plugin reports when the archive matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityRule {
    pub id: String,
    /// How sure the plugin is. Carried through to the manifest unchanged, and capped at 1.
    #[serde(default = "full")]
    pub confidence: f32,
    /// Every one of these must hold. An empty list never fires: a plugin that reported a
    /// capability for every archive would be reporting nothing.
    #[serde(default)]
    pub when: Vec<Match>,
}

fn full() -> f32 {
    1.0
}

/// A file a plugin says is a text resource, and what format it is in.
///
/// Only formats this build already reads *and writes* can be named, because the point of naming a
/// resource is that a translation reaches the game. A plugin cannot invent a format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRule {
    /// `*` stands for any run of characters, `?` for one. Matched against the whole entry name.
    pub pattern: String,
    pub format: String,
    /// Why this file is text, for whoever reads the plugin later.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl ResourceRule {
    /// The format this rule names, or `None` where the name is not one this build has.
    pub fn resolved(&self) -> Option<Format> {
        match self.format.as_str() {
            "properties" => Some(Format::Properties),
            "apple-strings" => Some(Format::AppleStrings),
            "android-strings" => Some(Format::AndroidStrings),
            "gettext" => Some(Format::Gettext),
            "ini" => Some(Format::Ini),
            "json" => Some(Format::Json),
            "renpy" => Some(Format::Renpy),
            "lines" => Some(Format::Lines),
            _ => None,
        }
    }
}

/// Where a plugin says a game keeps its glyph sheet, and how it is laid out (§16).
///
/// A hint, never a decision: the project still records which image its font is, because installing
/// the wrong sheet is a game that draws nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontHint {
    pub pattern: String,
    pub cell_width: u32,
    pub cell_height: u32,
    pub columns: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// One adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    pub id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Who wrote it, for a person deciding whether to trust what it says about their game.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<CapabilityRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resources: Vec<ResourceRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<FontHint>,
    /// Rules the plugin offers (§19). Off until somebody turns them on, like every other rule.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<crate::rules::Rule>,
    /// Terms this game or engine uses, added to the dictionary (§12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dictionary: Option<crate::dictionary::Pack>,
    /// Where it was loaded from. Not part of the file; filled in on load.
    #[serde(skip)]
    pub path: PathBuf,
}

impl Plugin {
    /// What is wrong with this plugin, in sentences a person can act on.
    ///
    /// Checked on load rather than at the moment a broken piece would have been used, because a
    /// plugin that quietly contributes nothing looks exactly like a plugin that had nothing to
    /// contribute, and the two need telling apart.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.id.trim().is_empty() {
            problems.push("it has no id".to_string());
        }
        for rule in &self.capabilities {
            if rule.when.is_empty() {
                problems.push(format!(
                    "the capability {} has nothing to match on, so it would be reported for every \
                     game",
                    rule.id
                ));
            }
            if rule.id.trim().is_empty() {
                problems.push("a capability has no id".to_string());
            }
        }
        for resource in &self.resources {
            if resource.resolved().is_none() {
                problems.push(format!(
                    "{} names the format {:?}, which this build does not have",
                    resource.pattern, resource.format
                ));
            }
        }
        for font in &self.fonts {
            if font.cell_width == 0 || font.cell_height == 0 || font.columns == 0 {
                problems.push(format!(
                    "the font hint for {} has a zero in its grid",
                    font.pattern
                ));
            }
        }
        problems
    }
}

/// Every plugin a project has, and what they contribute together.
#[derive(Debug, Clone, Default)]
pub struct Plugins {
    pub loaded: Vec<Plugin>,
    /// Files that would not parse, with the reason. Kept rather than raised: one unreadable
    /// plugin should not stop a project opening, and it must not disappear either.
    pub broken: Vec<(PathBuf, String)>,
}

impl Plugins {
    /// Loads every `.json` file in a directory. A missing directory is no plugins, not an error.
    pub fn load(dir: &Path) -> Result<Plugins> {
        let mut plugins = Plugins::default();
        if !dir.exists() {
            return Ok(plugins);
        }
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        // Sorted so two projects with the same plugins behave the same way: load order decides
        // which resource rule wins, and a directory listing is not an order.
        files.sort();

        for file in files {
            let text = match std::fs::read_to_string(&file) {
                Ok(text) => text,
                Err(e) => {
                    plugins.broken.push((file, e.to_string()));
                    continue;
                }
            };
            match serde_json::from_str::<Plugin>(&text) {
                Ok(mut plugin) => {
                    plugin.path = file;
                    plugins.loaded.push(plugin);
                }
                Err(e) => plugins.broken.push((file, e.to_string())),
            }
        }
        Ok(plugins)
    }

    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty() && self.broken.is_empty()
    }

    /// What the plugins say this archive is (§6).
    ///
    /// Every capability carries the plugin that reported it, because a detection a person did not
    /// expect has to be traceable to whoever claimed it.
    pub fn capabilities(&self, archive: &Archive) -> Vec<Capability> {
        let manifest = archive
            .get("META-INF/MANIFEST.MF")
            .map(|e| Manifest::parse(&String::from_utf8_lossy(&e.data)));

        let mut found = Vec::new();
        for plugin in &self.loaded {
            for rule in &plugin.capabilities {
                if rule.when.is_empty()
                    || !rule
                        .when
                        .iter()
                        .all(|m| m.holds(archive, manifest.as_ref()))
                {
                    continue;
                }
                let mut evidence = vec![format!("plugin {}", plugin.id)];
                evidence.extend(rule.when.iter().map(Match::evidence));
                found.push(Capability {
                    id: rule.id.clone(),
                    confidence: rule.confidence.clamp(0.0, 1.0),
                    evidence,
                });
            }
        }
        found
    }

    /// The formats plugins claim for files, in the shape `writeback` takes.
    pub fn formats(&self) -> Formats {
        let mut formats = Formats::default();
        for plugin in &self.loaded {
            for resource in &plugin.resources {
                if let Some(format) = resource.resolved() {
                    formats.claims.push(Claim {
                        pattern: resource.pattern.clone(),
                        format,
                        plugin: plugin.id.clone(),
                    });
                }
            }
        }
        formats
    }

    /// The rules every plugin offers, named so their source is visible in the interface.
    ///
    /// A rule changes how a game behaves, and one that arrived with a plugin should not read like
    /// one somebody in this project wrote. It stays off until switched on, as all rules do.
    pub fn rules(&self) -> Vec<crate::rules::Rule> {
        let mut rules = Vec::new();
        for plugin in &self.loaded {
            for rule in &plugin.rules {
                let mut copy = rule.clone();
                copy.id = format!("{}:{}", plugin.id, rule.id);
                copy.enabled = false;
                rules.push(copy);
            }
        }
        rules
    }

    /// The dictionary packs plugins bring (§12).
    pub fn dictionary_packs(&self) -> Vec<crate::dictionary::Pack> {
        self.loaded
            .iter()
            .filter_map(|p| p.dictionary.clone())
            .collect()
    }

    /// A plugin's guess at the grid of a glyph sheet, where one matches (§16).
    pub fn font_hint(&self, entry: &str) -> Option<&FontHint> {
        self.loaded
            .iter()
            .flat_map(|p| &p.fonts)
            .find(|hint| glob(&hint.pattern, entry))
    }
}

/// One plugin's claim that a file is a resource of a given format.
#[derive(Debug, Clone, PartialEq)]
pub struct Claim {
    pub pattern: String,
    pub format: Format,
    pub plugin: String,
}

/// The formats claimed by plugins, asked once per file by `writeback::plan`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Formats {
    pub claims: Vec<Claim>,
}

impl Formats {
    /// What a plugin says this file is, if one says anything.
    ///
    /// First claim wins, and load order is alphabetical by file: two plugins claiming the same
    /// file is a conflict somebody has to resolve, and picking the more specific pattern would
    /// hide it behind a rule nobody can see.
    pub fn of(&self, entry: &str) -> Option<&Claim> {
        self.claims.iter().find(|c| glob(&c.pattern, entry))
    }

    pub fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }
}

/// Shell-style matching: `*` for any run of characters, `?` for exactly one.
///
/// Not a regular expression, on purpose. A plugin file is written by hand by somebody who wants to
/// say "the files under `data/lang/`", and a pattern language with backtracking in it is a way for
/// that person to hang the tool on an archive with a few thousand entries in it.
pub fn glob(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();

    // The standard two-index walk with one remembered star, which is linear and cannot blow up.
    let (mut p, mut n) = (0usize, 0usize);
    let (mut star, mut matched) = (None, 0usize);

    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            matched = n;
            p += 1;
        } else if let Some(at) = star {
            p = at + 1;
            matched += 1;
            n = matched;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

/// Where a project keeps its plugins.
pub fn dir(root: &Path) -> PathBuf {
    root.join("plugins")
}
