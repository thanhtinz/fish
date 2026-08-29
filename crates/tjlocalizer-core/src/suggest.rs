//! Translation candidates (specification §22, "generate translation candidates").
//!
//! Candidates come from what the project already knows: its translation memory and its glossary.
//! Nothing here invents a translation, and nothing here approves one on its own - a candidate is
//! a proposal a human accepts or rejects. The one exception is an exact memory hit, which is by
//! definition a translation this project already approved for this exact string.

use crate::graph::ContentGraph;
use crate::vietnamese::{Glossary, TranslationMemory, TranslationStore};
use serde::{Deserialize, Serialize};

/// Where a candidate came from. Kept on the candidate so a reviewer can weigh it: an exact memory
/// hit and a 0.82-similar one deserve very different amounts of attention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    /// The same source string was translated before in this project.
    MemoryExact,
    /// A similar string was translated before.
    MemoryFuzzy { score: f32 },
    /// The whole string is a glossary term.
    GlossaryTerm,
}

/// One proposed translation for one node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub node_id: String,
    pub source: String,
    pub target: String,
    pub origin: Origin,
    /// True only when the candidate is a restatement of a decision already made, never when it is
    /// an inference. `apply_safe` uses this; a reviewer should read everything else.
    pub auto_approvable: bool,
    /// Glossary terms found inside the source that the reviewer should keep consistent. Present
    /// even on a memory candidate, because a remembered translation can predate a glossary change.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terms: Vec<String>,
}

/// The candidate set for a project, as written to translations/candidates.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CandidateSet {
    pub candidates: Vec<Candidate>,
    /// Translatable nodes with nothing to propose. Counted rather than listed, because on a first
    /// pass this is nearly every node and the number is the useful part.
    #[serde(default)]
    pub without_candidate: usize,
}

impl CandidateSet {
    pub fn auto_approvable(&self) -> impl Iterator<Item = &Candidate> {
        self.candidates.iter().filter(|c| c.auto_approvable)
    }
}

/// Proposes a translation for every translatable node that does not already have an approved one.
///
/// `fuzzy_threshold` is the similarity below which a memory entry is not worth showing at all.
pub fn candidates(
    graph: &ContentGraph,
    memory: &TranslationMemory,
    glossary: &Glossary,
    approved: &TranslationStore,
    fuzzy_threshold: f32,
) -> CandidateSet {
    let mut set = CandidateSet::default();

    for node in graph.translatable() {
        if approved.get(&node.id).is_some() {
            continue;
        }
        let source = node.source_text.as_str();
        let terms: Vec<String> = glossary
            .matches_in(source)
            .into_iter()
            .map(|e| e.source.clone())
            .collect();

        // A glossary entry for the whole string is a term decision, so it outranks a fuzzy memory
        // hit; a locked one outranks even an exact memory hit, since locking is how a project says
        // "this term is settled".
        let locked_term = glossary.lookup(source).filter(|e| e.locked);
        let candidate = if let Some(entry) = locked_term {
            Some(Candidate {
                node_id: node.id.clone(),
                source: source.to_string(),
                target: entry.target.clone(),
                origin: Origin::GlossaryTerm,
                auto_approvable: true,
                terms,
            })
        } else if let Some(hit) = memory.best(source, fuzzy_threshold) {
            let exact = hit.entry.source == source;
            Some(Candidate {
                node_id: node.id.clone(),
                source: source.to_string(),
                target: hit.entry.target.clone(),
                origin: if exact {
                    Origin::MemoryExact
                } else {
                    Origin::MemoryFuzzy { score: hit.score }
                },
                auto_approvable: exact,
                terms,
            })
        } else if let Some(entry) = glossary.lookup(source) {
            Some(Candidate {
                node_id: node.id.clone(),
                source: source.to_string(),
                target: entry.target.clone(),
                origin: Origin::GlossaryTerm,
                auto_approvable: false,
                terms,
            })
        } else {
            None
        };

        match candidate {
            Some(c) => set.candidates.push(c),
            None => set.without_candidate += 1,
        }
    }

    set
}

/// Approves only the candidates that restate an existing decision, and reports how many.
///
/// Everything else stays a proposal. This is what makes re-running the pipeline over an updated
/// game cheap without ever letting the tool translate on its own authority.
pub fn apply_safe(set: &CandidateSet, approved: &mut TranslationStore) -> usize {
    let mut applied = 0;
    for candidate in set.auto_approvable() {
        approved.set(&candidate.node_id, &candidate.target);
        applied += 1;
    }
    applied
}

/// Feeds every approved translation back into the memory, so the next game reuses this one's work.
pub fn learn(graph: &ContentGraph, approved: &TranslationStore, memory: &mut TranslationMemory) {
    for node in &graph.nodes {
        if let Some(target) = approved.get(&node.id) {
            memory.remember(
                &node.source_text,
                target,
                Some(format!("{:?}", node.context)),
            );
        }
    }
}
