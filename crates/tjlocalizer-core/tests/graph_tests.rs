//! Extraction and context classification.

use tjlocalizer_core::graph::{self, ContextType, TextSource};
use tjlocalizer_core::jar::Archive;

fn graph() -> tjlocalizer_core::graph::ContentGraph {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sample-game.jar"))
        .expect("fixture missing - run tools/make-fixtures.sh");
    graph::extract(&Archive::read(&bytes).unwrap())
}

#[test]
fn extracts_from_classes_and_resources() {
    let g = graph();
    let texts: Vec<&str> = g.nodes.iter().map(|n| n.source_text.as_str()).collect();

    assert!(texts.contains(&"Start Game"), "class literal missing");
    assert!(texts.contains(&"Green Field"), "properties value missing");

    let from_class = g
        .nodes
        .iter()
        .any(|n| matches!(n.source, TextSource::ClassConstant { .. }));
    let from_props = g
        .nodes
        .iter()
        .any(|n| matches!(n.source, TextSource::ResourceProperty { .. }));
    assert!(from_class && from_props);
}

#[test]
fn node_ids_are_stable_across_runs() {
    // Re-analysing an unchanged game must reproduce the same ids, or every re-import orphans the
    // translations already approved against it.
    let first: Vec<String> = graph().nodes.iter().map(|n| n.id.clone()).collect();
    let second: Vec<String> = graph().nodes.iter().map(|n| n.id.clone()).collect();
    assert_eq!(first, second);
    assert!(!first.is_empty());
}

#[test]
fn resource_paths_are_not_translatable() {
    let g = graph();
    let node = g
        .nodes
        .iter()
        .find(|n| n.source_text == "/img/hud.png")
        .expect("path literal missing");
    assert_eq!(node.context, ContextType::Technical);
    assert!(!node.context.is_translatable());
}

#[test]
fn classifies_by_shape_not_by_game() {
    for (text, expected) in [
        ("Quit", ContextType::Ui),
        ("Start Game", ContextType::Ui),
        ("You have arrived at last, traveller.", ContextType::Dialogue),
        ("HP: %d / %d", ContextType::Format),
        ("/img/hud.png", ContextType::Technical),
        ("com/example/Main", ContextType::Technical),
        ("hud.png", ContextType::Technical),
        ("()V", ContextType::Technical),
    ] {
        assert_eq!(graph::classify(text), expected, "for {text:?}");
    }
}

#[test]
fn finds_placeholders_that_must_survive_translation() {
    assert_eq!(graph::find_placeholders("HP: %d / %d"), vec!["%d", "%d"]);
    assert_eq!(graph::find_placeholders("Level {0} of {1}"), vec!["{0}", "{1}"]);
    assert_eq!(graph::find_placeholders("%s gained %02d points"), vec!["%s", "%02d"]);
    // A literal percent sign is not a placeholder and must not be reported as one.
    assert!(graph::find_placeholders("100%% complete").is_empty());
    assert!(graph::find_placeholders("no placeholders here").is_empty());
}

#[test]
fn placeholders_are_recorded_on_the_node() {
    let g = graph();
    let node = g
        .nodes
        .iter()
        .find(|n| n.source_text == "HP: %d / %d")
        .expect("format literal missing");
    assert_eq!(node.constraints.placeholders, vec!["%d", "%d"]);
    assert_eq!(node.context, ContextType::Format);
}

#[test]
fn skips_structural_pool_entries() {
    // Class names, descriptors and field names live in the same constant pool. If any of them
    // reached the graph a translator could rename them and break the class.
    let g = graph();
    for forbidden in ["SampleGame", "java/lang/String", "main", "TITLE", "([Ljava/lang/String;)V"] {
        assert!(
            !g.nodes.iter().any(|n| n.source_text == forbidden),
            "{forbidden:?} must not be extracted"
        );
    }
}

/// The manifest is the archive's structure, not the game's text.
///
/// Offering `MIDlet-1: Sample Game,/icon.png,SampleGame` for translation invites a translator to
/// rename the entry point, producing a build that installs and then will not start.
#[test]
fn the_manifest_is_never_offered_for_translation() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sample-game.jar"))
        .expect("fixture missing - run tools/make-fixtures.sh");
    let archive = tjlocalizer_core::jar::Archive::read(&bytes).unwrap();
    let graph = tjlocalizer_core::graph::extract(&archive);

    for node in &graph.nodes {
        assert!(
            !node.source_text.starts_with("MIDlet-")
                && !node.source_text.starts_with("MicroEdition-")
                && !node.source_text.starts_with("Manifest-Version"),
            "manifest line offered as game text: {:?}",
            node.source_text
        );
    }

    // The resource text that is real game content is still there.
    assert!(graph
        .translatable()
        .any(|n| n.source_text == "Green Field"));
}
