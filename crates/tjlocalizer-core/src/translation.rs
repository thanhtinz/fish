//! Glossary, translation memory and the approved store (specification §13).
//!
//! Nothing here is specific to a language. A glossary fixes terminology, a memory reuses wording
//! already approved, and the store holds what a human decided - all of which work the same way
//! whatever the pair of languages. The rules that *are* language-specific live in `quality` and
//! `register`.

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
pub fn similarity(a: &str, b: &str) -> f32 {
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

/// A problem found in a candidate translation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Issue {
    pub code: String,
    pub detail: String,
}

/// The store of approved translations for one target language, keyed by content-graph node id.
///
/// One store per language rather than one store with a language column: a project translating
/// into five languages has five independent bodies of work, reviewed and approved separately, and
/// merging them into one file would make a conflict in one language a conflict in all of them.
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
