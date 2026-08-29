//! Translation candidates: what may be approved automatically and what may not.

use tjlocalizer_core::graph::{ContentGraph, ContextType, Constraints, TextNode, TextSource};
use tjlocalizer_core::suggest::{apply_safe, candidates, learn, Origin};
use tjlocalizer_core::vietnamese::{Glossary, GlossaryEntry, TranslationMemory, TranslationStore};

fn node(id: &str, text: &str) -> TextNode {
    TextNode {
        id: id.to_string(),
        source: TextSource::ResourceProperty {
            resource: "ui.properties".to_string(),
            key: id.to_string(),
        },
        source_text: text.to_string(),
        source_encoding: None,
        context: ContextType::Ui,
        constraints: Constraints {
            placeholders: Vec::new(),
            source_len: text.chars().count(),
        },
    }
}

fn graph(pairs: &[(&str, &str)]) -> ContentGraph {
    ContentGraph {
        nodes: pairs.iter().map(|(id, text)| node(id, text)).collect(),
    }
}

#[test]
fn an_exact_memory_hit_may_be_approved_automatically() {
    let graph = graph(&[("a", "Start Game")]);
    let mut memory = TranslationMemory::default();
    memory.remember("Start Game", "Bắt đầu trò chơi", None);

    let set = candidates(
        &graph,
        &memory,
        &Glossary::default(),
        &TranslationStore::default(),
        0.75,
    );
    assert_eq!(set.candidates.len(), 1);
    assert_eq!(set.candidates[0].origin, Origin::MemoryExact);
    assert!(set.candidates[0].auto_approvable);

    let mut approved = TranslationStore::default();
    assert_eq!(apply_safe(&set, &mut approved), 1);
    assert_eq!(approved.get("a"), Some("Bắt đầu trò chơi"));
}

#[test]
fn a_fuzzy_memory_hit_is_offered_but_never_approved() {
    let graph = graph(&[("a", "Are you sure?")]);
    let mut memory = TranslationMemory::default();
    memory.remember("Are you sure!", "Bạn có chắc không?", None);

    let set = candidates(
        &graph,
        &memory,
        &Glossary::default(),
        &TranslationStore::default(),
        0.75,
    );
    assert_eq!(set.candidates.len(), 1);
    assert!(matches!(set.candidates[0].origin, Origin::MemoryFuzzy { .. }));
    assert!(
        !set.candidates[0].auto_approvable,
        "a near-match is a suggestion; approving it is how a memory quietly corrupts a project"
    );

    let mut approved = TranslationStore::default();
    assert_eq!(apply_safe(&set, &mut approved), 0);
    assert!(approved.is_empty());
}

#[test]
fn a_locked_glossary_term_outranks_the_memory() {
    let graph = graph(&[("a", "Mana")]);
    let mut memory = TranslationMemory::default();
    memory.remember("Mana", "Pháp lực", None);
    let glossary = Glossary {
        entries: vec![GlossaryEntry {
            source: "Mana".to_string(),
            target: "Nội lực".to_string(),
            locked: true,
            note: String::new(),
        }],
    };

    let set = candidates(&graph, &memory, &glossary, &TranslationStore::default(), 0.75);
    assert_eq!(set.candidates[0].origin, Origin::GlossaryTerm);
    assert_eq!(set.candidates[0].target, "Nội lực");
    assert!(set.candidates[0].auto_approvable);
}

#[test]
fn an_unlocked_glossary_term_is_a_proposal_only() {
    let graph = graph(&[("a", "Mana")]);
    let glossary = Glossary {
        entries: vec![GlossaryEntry {
            source: "Mana".to_string(),
            target: "Nội lực".to_string(),
            locked: false,
            note: String::new(),
        }],
    };

    let set = candidates(
        &graph,
        &TranslationMemory::default(),
        &glossary,
        &TranslationStore::default(),
        0.75,
    );
    assert_eq!(set.candidates[0].origin, Origin::GlossaryTerm);
    assert!(!set.candidates[0].auto_approvable);
}

#[test]
fn already_approved_nodes_are_left_alone() {
    let graph = graph(&[("a", "Quit")]);
    let mut memory = TranslationMemory::default();
    memory.remember("Quit", "Rời khỏi", None);
    let mut approved = TranslationStore::default();
    approved.set("a", "Thoát");

    let set = candidates(&graph, &memory, &Glossary::default(), &approved, 0.75);
    assert!(set.candidates.is_empty());
    assert_eq!(set.without_candidate, 0);

    // The human decision stands; the memory does not overwrite it.
    apply_safe(&set, &mut approved);
    assert_eq!(approved.get("a"), Some("Thoát"));
}

#[test]
fn nodes_with_nothing_to_propose_are_counted() {
    let graph = graph(&[("a", "Inventory"), ("b", "Shop")]);
    let set = candidates(
        &graph,
        &TranslationMemory::default(),
        &Glossary::default(),
        &TranslationStore::default(),
        0.75,
    );
    assert!(set.candidates.is_empty());
    assert_eq!(set.without_candidate, 2);
}

#[test]
fn terms_inside_a_string_are_flagged_for_the_reviewer() {
    let graph = graph(&[("a", "Restore Mana")]);
    let glossary = Glossary {
        entries: vec![GlossaryEntry {
            source: "Mana".to_string(),
            target: "Nội lực".to_string(),
            locked: true,
            note: String::new(),
        }],
    };
    let mut memory = TranslationMemory::default();
    memory.remember("Restore Mana", "Hồi Nội lực", None);

    let set = candidates(&graph, &memory, &glossary, &TranslationStore::default(), 0.75);
    assert_eq!(set.candidates[0].terms, vec!["Mana".to_string()]);
}

#[test]
fn approved_work_is_folded_back_into_the_memory() {
    let graph = graph(&[("a", "Start Game"), ("b", "Quit")]);
    let mut approved = TranslationStore::default();
    approved.set("a", "Bắt đầu trò chơi");

    let mut memory = TranslationMemory::default();
    learn(&graph, &approved, &mut memory);

    assert_eq!(memory.entries.len(), 1);
    assert_eq!(memory.exact("Start Game").unwrap().target, "Bắt đầu trò chơi");
    assert!(memory.exact("Quit").is_none(), "untranslated nodes teach nothing");
}
