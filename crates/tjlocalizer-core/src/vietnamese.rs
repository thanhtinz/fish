//! The Vietnamese language layer (specification §11-§13).
//!
//! This is deliberately not a machine translator. It is the part of the system that decides how
//! an already-chosen Vietnamese string should look: terminology fixed by the glossary, previously
//! approved wording reused from the memory, and normalisation and constraint checks applied so
//! the result is publishable rather than merely present.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A term whose translation is fixed for the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossaryEntry {
    pub source: String,
    pub target: String,
    /// When set, the term may not be overridden by a translator or by the memory.
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Glossary {
    pub entries: Vec<GlossaryEntry>,
}

impl Glossary {
    pub fn lookup(&self, source: &str) -> Option<&GlossaryEntry> {
        self.entries.iter().find(|e| e.source == source)
    }

    /// Terms from the glossary that appear inside `text`, longest first.
    ///
    /// Longest-first matters: with both "trang bị" and "cường hóa trang bị" in the glossary, the
    /// shorter term would otherwise match inside the longer one and produce the wrong reading.
    pub fn matches_in(&self, text: &str) -> Vec<&GlossaryEntry> {
        let mut found: Vec<&GlossaryEntry> = self
            .entries
            .iter()
            .filter(|e| text.contains(&e.source))
            .collect();
        found.sort_by_key(|e| std::cmp::Reverse(e.source.chars().count()));
        found
    }
}

/// An approved translation, keyed by source text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub context: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranslationMemory {
    pub entries: Vec<MemoryEntry>,
}

/// How closely a memory hit matches what is being translated.
#[derive(Debug, Clone)]
pub struct MemoryMatch<'a> {
    pub entry: &'a MemoryEntry,
    /// 1.0 for an exact match, lower for a fuzzy one.
    pub score: f32,
}

impl TranslationMemory {
    pub fn exact(&self, source: &str) -> Option<&MemoryEntry> {
        self.entries.iter().find(|e| e.source == source)
    }

    /// Best match at or above `threshold`, exact matches first.
    ///
    /// Fuzzy matching is offered as a *suggestion* only. Applying a 0.8-similar translation
    /// automatically is how a memory quietly corrupts a project: "Bạn có chắc không?" and
    /// "Bạn có chắc chứ?" are near-identical and mean the same, but "Mở khóa" and "Mở khoá" are
    /// near-identical and one of them is wrong.
    pub fn best(&self, source: &str, threshold: f32) -> Option<MemoryMatch<'_>> {
        if let Some(entry) = self.exact(source) {
            return Some(MemoryMatch { entry, score: 1.0 });
        }
        self.entries
            .iter()
            .map(|entry| MemoryMatch {
                score: similarity(source, &entry.source),
                entry,
            })
            .filter(|m| m.score >= threshold)
            .max_by(|a, b| a.score.total_cmp(&b.score))
    }

    pub fn remember(&mut self, source: &str, target: &str, context: Option<String>) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.source == source) {
            existing.target = target.to_string();
            existing.context = context;
            return;
        }
        self.entries.push(MemoryEntry {
            source: source.to_string(),
            target: target.to_string(),
            context,
        });
    }
}

/// Character-level similarity in the range 0.0 to 1.0, by normalised edit distance.
fn similarity(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    1.0 - (edit_distance(&a, &b) as f32 / longest as f32)
}

fn edit_distance(a: &[char], b: &[char]) -> usize {
    // Two rows rather than a full matrix: these are short UI strings, but a game can hold tens of
    // thousands and the memory is searched for every one of them.
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];

    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// Tidies Vietnamese text without changing its wording.
///
/// Only mechanical fixes: spacing around punctuation and collapsed whitespace. Anything that
/// would alter meaning or tone belongs to a translator, not to a normaliser.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_space = false;

    for c in text.trim().chars() {
        if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
            }
            last_was_space = true;
            continue;
        }
        // Vietnamese sets no space before these and one after, unlike French.
        if matches!(c, ',' | '.' | '!' | '?' | ';' | ':') {
            while out.ends_with(' ') {
                out.pop();
            }
        }
        out.push(c);
        last_was_space = false;
    }
    out
}

/// A problem found in a candidate translation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Issue {
    pub code: String,
    pub detail: String,
}

/// Checks a translation against its source before it can be approved.
pub fn check(source: &str, target: &str, placeholders: &[String]) -> Vec<Issue> {
    let mut issues = Vec::new();

    if target.trim().is_empty() {
        issues.push(Issue {
            code: "empty".into(),
            detail: "translation is empty".into(),
        });
        return issues;
    }

    // Every placeholder in the source must appear the same number of times in the target. A lost
    // "%d" is not a cosmetic problem: the runtime format call will be wrong or will throw.
    for placeholder in placeholders {
        let wanted = source.matches(placeholder.as_str()).count();
        let got = target.matches(placeholder.as_str()).count();
        if wanted != got {
            issues.push(Issue {
                code: "placeholder".into(),
                detail: format!("{placeholder} appears {got} times, expected {wanted}"),
            });
        }
    }

    if target != normalize(target) {
        issues.push(Issue {
            code: "spacing".into(),
            detail: "leading, trailing or duplicated whitespace".into(),
        });
    }

    // Vietnamese is usually longer than English, but a translation several times the original is
    // a sign of an explanation having been written into a UI label that has no room for it.
    let source_len = source.chars().count();
    let target_len = target.chars().count();
    if source_len > 0 && target_len > source_len * 3 && target_len > 24 {
        issues.push(Issue {
            code: "length".into(),
            detail: format!("{target_len} characters against a {source_len} character source"),
        });
    }

    issues
}

/// Applies locked glossary terms to a translation, reporting the ones that disagree.
///
/// Returns the issues rather than rewriting the text: a locked term appearing with the wrong
/// wording usually means the whole sentence was built around the wrong reading, and silently
/// substituting it would produce something ungrammatical.
pub fn check_glossary(target: &str, source: &str, glossary: &Glossary) -> Vec<Issue> {
    let mut issues = Vec::new();
    for entry in glossary.matches_in(source) {
        if entry.locked && !target.contains(&entry.target) {
            issues.push(Issue {
                code: "glossary".into(),
                detail: format!(
                    "source contains the locked term {:?}, which must be translated as {:?}",
                    entry.source, entry.target
                ),
            });
        }
    }
    issues
}

/// The store of approved translations for a project, keyed by content-graph node id.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranslationStore {
    #[serde(default)]
    pub approved: BTreeMap<String, String>,
}

impl TranslationStore {
    pub fn get(&self, node_id: &str) -> Option<&str> {
        self.approved.get(node_id).map(|s| s.as_str())
    }

    pub fn set(&mut self, node_id: impl Into<String>, target: impl Into<String>) {
        self.approved.insert(node_id.into(), target.into());
    }

    pub fn len(&self) -> usize {
        self.approved.len()
    }

    pub fn is_empty(&self) -> bool {
        self.approved.is_empty()
    }
}
