//! Multilingual dictionaries (specification §12).
//!
//! A dictionary here is not a general one. It is a *game* dictionary: entries carry a domain, so
//! that `攻击` reads as "tấn công" in combat text and `Guild` as "bang hội" rather than "hiệp hội".
//! That domain tagging is most of what separates a translation that feels like a game from one
//! that reads like a manual, and it is why this is worth having even though it cannot translate a
//! sentence.
//!
//! What it deliberately does not do is claim to translate prose. It resolves terms. `translate`
//! elsewhere decides what to do with the terms it resolved.

use crate::lang::Language;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The part of a game a term belongs to.
///
/// A term can mean different things in different parts of the same game - `Skill` in a menu is a
/// heading, in combat text it is what was just used - so the domain is matched against the
/// content node's context before a term is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    /// Menus, buttons, labels.
    Ui,
    /// Combat, damage, status effects.
    Combat,
    /// Items, equipment, materials.
    Item,
    /// Skills, spells, techniques.
    Skill,
    /// Quests, missions, objectives.
    Quest,
    /// Guilds, friends, chat, trade.
    Social,
    /// Stats and numbers: HP, MP, EXP, level.
    Stat,
    /// System messages, errors, network.
    System,
    /// Story and dialogue vocabulary, including the register-carrying words.
    Story,
    /// Not tied to any part of the game.
    General,
}

impl Domain {
    /// How well this domain suits a content node's context, 0.0 to 1.0.
    ///
    /// `General` never scores highest but always scores something, so a general term is used when
    /// no domain-specific one exists rather than nothing being offered.
    pub fn affinity(self, context: &str) -> f32 {
        if self == Domain::General {
            return 0.55;
        }
        let matches = matches!(
            (self, context),
            (Domain::Ui, "ui")
                | (Domain::Combat, "dialogue")
                | (Domain::Item, "item")
                | (Domain::Skill, "skill")
                | (Domain::Quest, "quest")
                | (Domain::Social, "ui")
                | (Domain::Stat, "format")
                | (Domain::System, "system")
                | (Domain::Story, "story")
                | (Domain::Story, "dialogue")
                | (Domain::Ui, "tutorial")
        );
        if matches {
            1.0
        } else {
            0.7
        }
    }
}

/// One dictionary entry: a term in one language and its reading in another.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub source: String,
    pub target: String,
    pub domain: Domain,
    /// Raised for a reading that should win when several apply. Defaults to zero.
    #[serde(default)]
    pub priority: i32,
    /// Why this reading, for a translator deciding whether to accept it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// The entries for one direction, `from` to `to`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub from: Language,
    pub to: Language,
    /// Where the readings came from, so a disagreement can be traced.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_note: String,
    pub entries: Vec<Entry>,
}

/// A resolved term, with why it was chosen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub source: String,
    pub target: String,
    pub domain: Domain,
    /// 0.0 to 1.0: how well the entry's domain matched the context it is being used in.
    pub fit: f32,
    pub note: String,
}

/// One stretch of the source text, either a term that was resolved or text that was not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Segment {
    /// A dictionary term.
    Term { text: String, reading: Reading },
    /// A placeholder, a number, or punctuation - carried through untouched.
    Literal { text: String },
    /// Text no entry covered. This is the part a dictionary cannot translate, and naming it is
    /// the point: it is what tells the caller the gloss is incomplete.
    Unknown { text: String },
}

impl Segment {
    pub fn text(&self) -> &str {
        match self {
            Segment::Term { text, .. } | Segment::Literal { text } | Segment::Unknown { text } => {
                text
            }
        }
    }
}

/// Every loaded pack, indexed by direction.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dictionary {
    pub packs: Vec<Pack>,
}

impl Dictionary {
    pub fn add(&mut self, pack: Pack) {
        self.packs.push(pack);
    }

    /// Directions this dictionary can work in.
    pub fn directions(&self) -> Vec<(Language, Language)> {
        let mut seen: Vec<(Language, Language)> = Vec::new();
        for pack in &self.packs {
            let pair = (pack.from.clone(), pack.to.clone());
            if !seen.contains(&pair) {
                seen.push(pair);
            }
        }
        seen
    }

    pub fn entry_count(&self) -> usize {
        self.packs.iter().map(|p| p.entries.len()).sum()
    }

    /// Entries usable for this direction, matching on language rather than exact tag so a `vi`
    /// pack serves a `vi-VN` project.
    fn entries_for(&self, from: &Language, to: &Language) -> Vec<&Entry> {
        self.packs
            .iter()
            .filter(|p| p.from.same_language_as(from) && p.to.same_language_as(to))
            .flat_map(|p| p.entries.iter())
            .collect()
    }

    /// The best reading for an exact term.
    pub fn lookup(
        &self,
        term: &str,
        from: &Language,
        to: &Language,
        context: &str,
    ) -> Option<Reading> {
        let folded = fold(term);
        self.entries_for(from, to)
            .into_iter()
            .filter(|e| fold(&e.source) == folded)
            .map(|e| (score(e, context), e))
            // A tie goes to the first entry listed rather than the last, so a pack that adds a
            // second reading for a term does not silently change what the first one meant.
            .reduce(|best, next| if next.0 > best.0 { next } else { best })
            .map(|(_, e)| Reading {
                source: e.source.clone(),
                target: e.target.clone(),
                domain: e.domain,
                fit: fit_of(e, context),
                note: e.note.clone(),
            })
    }

    /// Every reading for a term, best first.
    ///
    /// `lookup` answers "what should this be"; this answers "what else could it be", which is a
    /// different question and the one asked by anybody trying to make a label fit. A dictionary
    /// carrying two words for one term is carrying a choice, and the shorter one is sometimes the
    /// one a button needs.
    pub fn readings(
        &self,
        term: &str,
        from: &Language,
        to: &Language,
        context: &str,
    ) -> Vec<Reading> {
        let folded = fold(term);
        let mut found: Vec<(f32, Reading)> = self
            .entries_for(from, to)
            .into_iter()
            .filter(|e| fold(&e.source) == folded)
            .map(|e| (score(e, context), e))
            .map(|(rank, e)| {
                (
                    rank,
                    Reading {
                        source: e.source.clone(),
                        target: e.target.clone(),
                        domain: e.domain,
                        fit: fit_of(e, context),
                        note: e.note.clone(),
                    },
                )
            })
            .collect();
        found.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut found: Vec<Reading> = found.into_iter().map(|(_, reading)| reading).collect();
        found.dedup_by(|a, b| a.target == b.target);
        found
    }

    /// Splits the text into terms, literals and unresolved stretches.
    ///
    /// Longest match wins. With both `攻击` and `攻击力` present, matching the shorter one first
    /// would leave a stray `力` and produce nonsense; the same applies to "Guild" inside
    /// "Guild Master".
    pub fn segment(
        &self,
        text: &str,
        from: &Language,
        to: &Language,
        context: &str,
    ) -> Vec<Segment> {
        let entries = self.entries_for(from, to);
        if entries.is_empty() {
            return vec![Segment::Unknown {
                text: text.to_string(),
            }];
        }

        // Group by folded source so several readings of one term are considered together, and
        // walk longest-first.
        let mut by_term: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
        for entry in entries {
            by_term.entry(fold(&entry.source)).or_default().push(entry);
        }
        let mut terms: Vec<&String> = by_term.keys().collect();
        terms.sort_by_key(|t| std::cmp::Reverse(t.chars().count()));

        let chars: Vec<char> = text.chars().collect();
        let folded: Vec<char> = fold(text).chars().collect();
        // Folding must not change the character count, or the offsets below would not line up.
        let aligned = folded.len() == chars.len();

        let mut segments: Vec<Segment> = Vec::new();
        let mut pending = String::new();
        let mut i = 0;

        while i < chars.len() {
            let mut matched = false;
            if aligned {
                for term in &terms {
                    let term_chars: Vec<char> = term.chars().collect();
                    if term_chars.is_empty() || i + term_chars.len() > chars.len() {
                        continue;
                    }
                    if folded[i..i + term_chars.len()] != term_chars[..] {
                        continue;
                    }
                    // In a spaced script a term must not match inside a longer word: "art" in
                    // "start" is not the term.
                    if from.script().uses_spaces_between_words()
                        && !at_word_boundary(&chars, i, term_chars.len())
                    {
                        continue;
                    }

                    let best = by_term[*term]
                        .iter()
                        .map(|e| (score(e, context), *e))
                        .max_by(|a, b| a.0.total_cmp(&b.0))
                        .expect("group is never empty");

                    flush(&mut pending, &mut segments);
                    let text: String = chars[i..i + term_chars.len()].iter().collect();
                    segments.push(Segment::Term {
                        text,
                        reading: Reading {
                            source: best.1.source.clone(),
                            target: best.1.target.clone(),
                            domain: best.1.domain,
                            fit: best.0,
                            note: best.1.note.clone(),
                        },
                    });
                    i += term_chars.len();
                    matched = true;
                    break;
                }
            }
            if !matched {
                pending.push(chars[i]);
                i += 1;
            }
        }
        flush(&mut pending, &mut segments);
        segments
    }
}

/// Splits accumulated text into literals (placeholders, numbers, punctuation) and unknown words.
///
/// Separating them matters: a stretch that is only `%d` or ` - ` is carried through and does not
/// mean the gloss is incomplete, while an unresolved word does.
fn flush(pending: &mut String, segments: &mut Vec<Segment>) {
    if pending.is_empty() {
        return;
    }
    let taken = std::mem::take(pending);
    if is_carried_through(&taken) {
        segments.push(Segment::Literal { text: taken });
    } else {
        segments.push(Segment::Unknown { text: taken });
    }
}

/// Text with nothing to translate: spacing, digits, punctuation, format placeholders.
///
/// Placeholders have to be recognised here rather than left to the generic test, because `%d` and
/// `{0}` contain letters. Without this, `HP: %d / %d` reads as half unresolved, and a caller
/// weighing coverage concludes the gloss is incomplete when in fact nothing was left out.
fn is_carried_through(text: &str) -> bool {
    let mut rest = text.to_string();
    for placeholder in crate::graph::find_placeholders(text) {
        rest = rest.replace(&placeholder, "");
    }
    !text.is_empty()
        && rest
            .chars()
            .all(|c| c.is_whitespace() || c.is_ascii_digit() || c.is_ascii_punctuation())
}

/// Whether a match at `start` of `len` characters stands as its own word.
fn at_word_boundary(chars: &[char], start: usize, len: usize) -> bool {
    let before_ok = start == 0 || !chars[start - 1].is_alphanumeric();
    let end = start + len;
    let after_ok = end >= chars.len() || !chars[end].is_alphanumeric();
    before_ok && after_ok
}

/// How well an entry suits the context: its domain's affinity, nudged by its priority.
///
/// Not clamped. It used to be, and that quietly disabled `priority` exactly where a curator most
/// needs it: a Ui entry in a "ui" context already scores 1.0, so adding its priority and clamping
/// gave every reading of the term the same number, and which one won came down to the order they
/// happened to be listed in. `fit` is clamped where it is reported, because there it is a
/// confidence a person reads; ranking uses the real number.
fn score(entry: &Entry, context: &str) -> f32 {
    entry.domain.affinity(context) + entry.priority as f32 * 0.05
}

/// The score as a confidence to show somebody: the same ranking, bounded.
fn fit_of(entry: &Entry, context: &str) -> f32 {
    score(entry, context).clamp(0.0, 1.0)
}

/// Case folding for matching. Deliberately character-preserving: the segmenter maps folded
/// offsets back onto the original text, so anything that changed the character count - Unicode
/// case folding of ß, say - would misalign the result. `to_lowercase` on ASCII is one-to-one, and
/// for anything else the check in `segment` falls back to no matching rather than a wrong one.
fn fold(text: &str) -> String {
    text.chars().flat_map(|c| c.to_lowercase()).collect()
}
