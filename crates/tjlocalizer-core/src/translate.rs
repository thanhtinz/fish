//! Producing a proposal for a line (specification §14, §22 step 9).
//!
//! This module is where the honesty of the whole tool is decided, so it is worth being plain
//! about what a dictionary can and cannot do.
//!
//! It *can* resolve terminology, and terminology is most of what makes a game translation read
//! like a game: `装备` as "trang bị" and not "thiết bị", `Guild` as "bang hội" and not "hiệp hội",
//! `EXP` as "kinh nghiệm". It can carry placeholders through untouched, apply the project's
//! register, and reuse wording already approved. For short interface strings - which are the
//! majority of a J2ME game's text - that is a complete answer.
//!
//! It *cannot* translate a sentence. Word order, agreement, classifiers and idiom are not in a
//! dictionary, and stitching readings together in source order produces something that looks like
//! a translation and is not one. So a proposal that did not resolve everything is returned as a
//! **gloss**, marked as needing a person, with the parts it could not resolve named. The one
//! thing this module will never do is present a stitched-together gloss as a finished line.
//!
//! A `Provider` is the seam for an engine that *can* translate sentences. None is built in: one
//! that called a network service would send the game's text to a third party, which is the user's
//! decision to make and their key to supply.

use crate::dictionary::{Dictionary, Segment};
use crate::lang::Language;
use crate::register::{Speaker, Stance, StyleProfile};
use crate::translation::{Glossary, TranslationMemory};
use serde::{Deserialize, Serialize};

/// What is being translated, and everything known about it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub source_text: String,
    pub from: Language,
    pub to: Language,
    /// The content graph's classification: `ui`, `dialogue`, `format`, and so on.
    pub context: String,
    pub placeholders: Vec<String>,
    #[serde(default)]
    pub speaker: Speaker,
    #[serde(default)]
    pub stance: Stance,
}

/// How complete a proposal is. The interface colours by this and, more importantly, the
/// auto-approval rule keys off it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    /// Every part of the source was resolved and the result stands on its own. Only reached for
    /// text that is entirely terms, placeholders and punctuation - which is what most interface
    /// strings are.
    Complete,
    /// Some parts resolved and some did not. Useful to a translator as a starting point; not a
    /// translation.
    Partial,
    /// Nothing resolved.
    None,
}

/// A proposed translation and an account of how it was arrived at.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub target_text: String,
    pub completeness: Completeness,
    /// 0.0 to 1.0. Never 1.0 from the dictionary engine alone: a complete term-for-term rendering
    /// of a phrase can still be the wrong phrasing.
    pub confidence: f32,
    /// Which engine produced this, for a translator weighing it.
    pub engine: String,
    /// The terms that were resolved, with their domains.
    pub terms: Vec<ResolvedTerm>,
    /// Stretches of the source nothing covered. Empty when `Complete`.
    pub unresolved: Vec<String>,
    /// The register this was produced under, so a reviewer can see what it was aiming at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub register: Option<String>,
    /// Plain-language notes for the reviewer.
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Proposal {
    /// Whether this may be approved without a person looking at it.
    ///
    /// Deliberately narrow. A dictionary gloss is never approvable on its own, however complete:
    /// "Start Game" resolving fully to "Bắt đầu trò chơi" is right, and the same machinery
    /// resolving "Are you sure?" to a term-by-term string is not, and nothing in the output tells
    /// them apart. What makes a proposal approvable is a human decision recorded earlier - an
    /// exact memory hit or a locked glossary term - and that lives in `suggest`.
    pub fn is_approvable(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTerm {
    pub source: String,
    pub target: String,
    pub domain: crate::dictionary::Domain,
    pub fit: f32,
}

/// An engine that can propose a translation.
///
/// Implemented here by the dictionary. The trait exists so an external engine can be added
/// without the rest of the system knowing which one it is - the same reason capabilities are
/// detected rather than games named.
pub trait Provider {
    /// A short name, shown to the reviewer beside the proposal.
    fn name(&self) -> &str;

    /// Whether this engine handles the direction at all.
    fn supports(&self, from: &Language, to: &Language) -> bool;

    fn propose(&self, request: &Request) -> Option<Proposal>;
}

/// The offline engine: dictionary terms, glossary, register.
///
/// Deterministic and local. It never reaches the network, so a project can be localized with no
/// account, no key and nothing leaving the machine.
pub struct DictionaryProvider<'a> {
    pub dictionary: &'a Dictionary,
    pub glossary: &'a Glossary,
    pub style: Option<&'a StyleProfile>,
}

impl<'a> DictionaryProvider<'a> {
    pub fn new(dictionary: &'a Dictionary, glossary: &'a Glossary) -> Self {
        Self {
            dictionary,
            glossary,
            style: None,
        }
    }

    pub fn with_style(mut self, style: &'a StyleProfile) -> Self {
        self.style = Some(style);
        self
    }
}

impl Provider for DictionaryProvider<'_> {
    fn name(&self) -> &str {
        "dictionary"
    }

    fn supports(&self, from: &Language, to: &Language) -> bool {
        self.dictionary
            .directions()
            .iter()
            .any(|(f, t)| f.same_language_as(from) && t.same_language_as(to))
    }

    fn propose(&self, request: &Request) -> Option<Proposal> {
        if !self.supports(&request.from, &request.to) {
            return None;
        }

        let segments = self.dictionary.segment(
            &request.source_text,
            &request.from,
            &request.to,
            &request.context,
        );

        // The project's glossary outranks the dictionary: the glossary is this project's own
        // decision and the dictionary is a general reading. It has to be applied against the
        // *source* of each segment, not against the rendered output - by then "Quit" has already
        // become "thoát" and there is nothing left for the glossary to match on.
        let glossary_for = |source: &str| -> Option<&crate::translation::GlossaryEntry> {
            self.glossary
                .entries
                .iter()
                .find(|e| e.source.eq_ignore_ascii_case(source) || e.source == source)
        };

        let mut rendered = String::new();
        let mut terms = Vec::new();
        let mut unresolved = Vec::new();

        for segment in &segments {
            match segment {
                Segment::Term { text, reading } => {
                    match glossary_for(text).or_else(|| glossary_for(&reading.source)) {
                        Some(entry) => {
                            rendered.push_str(&entry.target);
                            terms.push(ResolvedTerm {
                                source: entry.source.clone(),
                                target: entry.target.clone(),
                                domain: crate::dictionary::Domain::General,
                                fit: 1.0,
                            });
                        }
                        None => {
                            rendered.push_str(&reading.target);
                            terms.push(ResolvedTerm {
                                source: reading.source.clone(),
                                target: reading.target.clone(),
                                domain: reading.domain,
                                fit: reading.fit,
                            });
                        }
                    }
                }
                Segment::Literal { text } => rendered.push_str(text),
                Segment::Unknown { text } => {
                    // A glossary term the dictionary did not know still applies here.
                    let mut remaining = text.clone();
                    let mut covered = false;
                    for entry in self.glossary.matches_in(text) {
                        if remaining.contains(&entry.source) {
                            remaining = remaining.replace(&entry.source, &entry.target);
                            terms.push(ResolvedTerm {
                                source: entry.source.clone(),
                                target: entry.target.clone(),
                                domain: crate::dictionary::Domain::General,
                                fit: 1.0,
                            });
                            covered = true;
                        }
                    }
                    rendered.push_str(&remaining);
                    // Left in the source language rather than dropped. A gap a reviewer can see
                    // is recoverable; a silently deleted clause is not.
                    let trimmed = text.trim();
                    if !covered && !trimmed.is_empty() {
                        unresolved.push(trimmed.to_string());
                    }
                }
            }
        }

        let rendered = crate::quality::normalize(&rendered, &request.to);
        if rendered.is_empty() {
            return None;
        }

        // A gloss that resolved only a fraction of the string is worse than none. "Dragon Quest
        // Online" is a title, and substituting the one word the dictionary happens to know gives
        // "Dragon nhiệm vụ Online" - which looks like an attempt at a translation, invites a tired
        // reviewer to accept it, and is wrong in a way the original was not. Below half covered,
        // this engine has nothing useful to say and says nothing.
        let coverage = coverage_of(&segments);
        if coverage < 0.5 {
            return None;
        }

        let completeness = if terms.is_empty() {
            Completeness::None
        } else if unresolved.is_empty() {
            Completeness::Complete
        } else {
            Completeness::Partial
        };

        let mut notes = Vec::new();
        if completeness != Completeness::Complete {
            notes.push(format!(
                "{} part(s) of the source have no dictionary reading and were left as they were - \
                 this is a gloss, not a translation",
                unresolved.len()
            ));
        }
        if let Some(style) = self.style {
            for issue in style.check(&rendered) {
                notes.push(issue.detail);
            }
        }

        // Even a fully resolved phrase is capped well below certainty: right words in source order
        // is not the same as right phrasing.
        let average_fit = if terms.is_empty() {
            0.0
        } else {
            terms.iter().map(|t| t.fit).sum::<f32>() / terms.len() as f32
        };
        let confidence = (average_fit * coverage * 0.8).clamp(0.0, 0.8);

        Some(Proposal {
            target_text: rendered,
            completeness,
            confidence,
            engine: "dictionary".into(),
            terms,
            unresolved,
            register: self.style.map(|s| s.id.clone()),
            notes,
        })
    }
}

/// The share of the source, by characters, that was resolved or carried through.
fn coverage_of(segments: &[Segment]) -> f32 {
    let total: usize = segments.iter().map(|s| s.text().chars().count()).sum();
    if total == 0 {
        return 0.0;
    }
    let covered: usize = segments
        .iter()
        .filter(|s| !matches!(s, Segment::Unknown { .. }))
        .map(|s| s.text().chars().count())
        .sum();
    covered as f32 / total as f32
}

/// Runs the providers in order and returns the first proposal, then the memory's own view.
///
/// The memory is consulted first deliberately: wording a human already approved for this exact
/// string beats anything an engine can produce for it.
pub fn propose(
    request: &Request,
    memory: &TranslationMemory,
    providers: &[&dyn Provider],
) -> Option<Proposal> {
    if let Some(entry) = memory.exact(&request.source_text) {
        return Some(Proposal {
            target_text: entry.target.clone(),
            completeness: Completeness::Complete,
            confidence: 1.0,
            engine: "memory".into(),
            terms: Vec::new(),
            unresolved: Vec::new(),
            register: None,
            notes: vec!["this exact string was approved before in this project".into()],
        });
    }

    providers
        .iter()
        .filter(|p| p.supports(&request.from, &request.to))
        .find_map(|p| p.propose(request))
}
