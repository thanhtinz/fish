//! The joining the view models do.
//!
//! These are worth testing because they are where a display bug becomes a correctness bug: a row
//! showing no issue on a translation that lost a placeholder is a build that fails later, and a
//! fuzzy candidate shown as auto-approvable is the rule the core exists to enforce, quietly undone
//! by the interface.

use tjlocalizer_core::graph::{Constraints, ContextType, TextNode, TextSource};
use tjlocalizer_core::suggest::{Candidate, Origin};
use tjlocalizer_desktop_lib::state::NodeView;

fn node(text: &str, placeholders: &[&str], context: ContextType) -> TextNode {
    TextNode {
        id: "n1".into(),
        source: TextSource::ClassConstant {
            class: "Main.class".into(),
            utf8_index: 7,
            string_index: 8,
        },
        source_text: text.into(),
        source_encoding: None,
        context,
        constraints: Constraints {
            placeholders: placeholders.iter().map(|p| p.to_string()).collect(),
            source_len: text.chars().count(),
        },
    }
}

#[test]
fn a_lost_placeholder_is_reported_as_blocking() {
    let n = node("HP: %d / %d", &["%d", "%d"], ContextType::Format);
    let view = NodeView::of(&n, Some("Sinh lực"), None);

    let issue = view
        .issues
        .iter()
        .find(|i| i.code == "placeholder")
        .expect("dropping %d must be reported");
    assert!(
        issue.blocking,
        "a lost placeholder breaks the game at runtime, so it cannot be a soft warning"
    );
}

#[test]
fn a_sound_translation_has_no_issues() {
    let n = node("HP: %d / %d", &["%d", "%d"], ContextType::Format);
    let view = NodeView::of(&n, Some("Sinh lực: %d / %d"), None);
    assert!(view.issues.is_empty(), "{:?}", view.issues);
}

#[test]
fn an_untranslated_row_is_not_flagged() {
    let n = node("Start Game", &[], ContextType::Ui);
    let view = NodeView::of(&n, None, None);
    assert!(view.issues.is_empty());
    assert_eq!(view.target, None);
    assert!(view.translatable);
}

#[test]
fn a_fuzzy_candidate_never_reaches_the_interface_as_auto_approvable() {
    let n = node("Are you sure?", &[], ContextType::Ui);
    let candidate = Candidate {
        node_id: "n1".into(),
        source: "Are you sure?".into(),
        target: "Bạn có chắc không?".into(),
        origin: Origin::MemoryFuzzy { score: 0.86 },
        auto_approvable: false,
        terms: Vec::new(),
    };
    let view = NodeView::of(&n, None, Some(&candidate));
    let shown = view.candidate.expect("the candidate should be offered");

    assert_eq!(shown.origin, "memory-fuzzy");
    assert_eq!(shown.score, Some(0.86));
    assert!(!shown.auto_approvable);
}

#[test]
fn an_exact_memory_candidate_is_marked_auto_approvable() {
    let n = node("Quit", &[], ContextType::Ui);
    let candidate = Candidate {
        node_id: "n1".into(),
        source: "Quit".into(),
        target: "Thoát".into(),
        origin: Origin::MemoryExact,
        auto_approvable: true,
        terms: Vec::new(),
    };
    let shown = NodeView::of(&n, None, Some(&candidate)).candidate.unwrap();
    assert_eq!(shown.origin, "memory");
    assert_eq!(shown.score, None);
    assert!(shown.auto_approvable);
}

#[test]
fn technical_nodes_are_marked_untranslatable_rather_than_hidden() {
    let n = node("/img/hud.png", &[], ContextType::Technical);
    let view = NodeView::of(&n, None, None);
    assert!(
        !view.translatable,
        "a resource path must not be offered for translation"
    );
    assert_eq!(view.context, "technical");
}

#[test]
fn a_class_constant_reports_where_it_lives() {
    let n = node("Quit", &[], ContextType::Ui);
    let view = NodeView::of(&n, None, None);
    assert_eq!(view.location.kind, "class");
    assert_eq!(view.location.file, "Main.class");
    assert_eq!(view.location.detail, "constant #7");
}
