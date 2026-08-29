//! Reading a line from the lines around it (§10, §5, §15).
//!
//! The reason this module exists is that `Yes` is two characters and no punctuation, so on its
//! own it is a button - and it is also half the answers in any conversation. These tests are
//! mostly about the difference, and about the cases where the surroundings say nothing and the
//! honest answer is silence.

use tjlocalizer_core::context::{infer, speaker_prefix};
use tjlocalizer_core::graph::{Constraints, ContentGraph, ContextType, TextNode, TextSource};
use tjlocalizer_core::register::{Speaker, Stance};

fn class_node(class: &str, index: u16, text: &str) -> TextNode {
    TextNode {
        id: format!("{class}-{index}"),
        source: TextSource::ClassConstant {
            class: class.into(),
            utf8_index: index,
            string_index: index,
        },
        source_text: text.to_string(),
        source_encoding: None,
        context: tjlocalizer_core::graph::classify(text),
        constraints: Constraints {
            placeholders: tjlocalizer_core::graph::find_placeholders(text),
            source_len: text.chars().count(),
        },
    }
}

fn key_node(resource: &str, key: &str, text: &str) -> TextNode {
    TextNode {
        id: format!("{resource}-{key}"),
        source: TextSource::ResourceProperty {
            resource: resource.into(),
            key: key.into(),
        },
        source_text: text.to_string(),
        source_encoding: None,
        context: tjlocalizer_core::graph::classify(text),
        constraints: Constraints {
            placeholders: tjlocalizer_core::graph::find_placeholders(text),
            source_len: text.chars().count(),
        },
    }
}

#[test]
fn a_line_between_two_lines_of_dialogue_is_dialogue() {
    let graph = ContentGraph {
        nodes: vec![
            class_node("Talk", 1, "Have you seen the blacksmith today?"),
            class_node("Talk", 2, "Yes"),
            class_node("Talk", 3, "He was in the square this morning."),
        ],
    };
    // On its own the middle string is a button, and that is what the graph called it.
    assert_eq!(graph.nodes[1].context, ContextType::Ui);

    let inference = infer(&graph);
    let reading = inference.reading("Talk-2").expect("no reading");
    assert_eq!(reading.context, Some(ContextType::Dialogue));
    assert!(reading.why[0].contains("either side"), "{:?}", reading.why);
}

/// One neighbour is a coincidence. A rule that fired on it would relabel the last string of a
/// menu from the first string of the conversation after it.
#[test]
fn one_neighbour_is_not_enough() {
    let graph = ContentGraph {
        nodes: vec![
            class_node("Screen", 1, "Are you sure you want to quit the game?"),
            class_node("Screen", 2, "Yes"),
        ],
    };
    assert_eq!(infer(&graph).reading("Screen-2"), None);
}

/// Neighbours that disagree say nothing, which is different from saying "unknown".
#[test]
fn neighbours_that_disagree_settle_nothing() {
    let graph = ContentGraph {
        nodes: vec![
            class_node("Mixed", 1, "The gate creaks open before you."),
            class_node("Mixed", 2, "Yes"),
            class_node("Mixed", 3, "Iron Sword"),
        ],
    };
    let inference = infer(&graph);
    assert!(inference
        .reading("Mixed-2")
        .and_then(|r| r.context)
        .is_none());
}

#[test]
fn a_named_speaker_is_a_character_and_the_line_is_dialogue() {
    let graph = ContentGraph {
        nodes: vec![
            class_node("Town", 1, "Blacksmith: I can mend that for you."),
            class_node("Town", 2, "Blacksmith: Come back when you have the iron."),
            class_node("Guard", 3, "Guard: Move along, please."),
        ],
    };
    let inference = infer(&graph);

    let reading = inference.reading("Town-1").unwrap();
    assert_eq!(reading.speaker, Some(Speaker::Npc));
    assert_eq!(reading.character.as_deref(), Some("Blacksmith"));
    assert_eq!(
        reading.spoken_text.as_deref(),
        Some("I can mend that for you.")
    );
    assert_eq!(inference.voice("Town-1").0, Speaker::Npc);

    let smith = inference
        .cast
        .iter()
        .find(|c| c.name == "Blacksmith")
        .expect("no blacksmith");
    assert_eq!(smith.lines, 2);
    assert_eq!(smith.appears_in, vec!["Town"]);
    // Two characters in different files are not beside each other.
    assert!(smith.beside.is_empty());

    let guard = inference.cast.iter().find(|c| c.name == "Guard").unwrap();
    let hint = guard.suggested_stance.as_ref().expect("no stance hint");
    assert_eq!(hint.stance, Stance::Deferential);
    assert_eq!(hint.because, vec!["please"]);
}

/// The half of this module that earns the other half: a colon is in far more things than speech,
/// and reading `HP` as a character would put a fictional cast in front of a translator.
#[test]
fn a_colon_is_not_a_speaker() {
    for text in [
        "HP: 20",
        "Time: 3:00",
        "Score: %d",
        "level: 4",
        "http://example.com/thing",
        "Gold: 5",
        "ERROR_CODE: 12",
        "A very long name that nobody would ever call a character: hello there",
    ] {
        assert_eq!(speaker_prefix(text), None, "{text:?} read as speech");
    }

    assert_eq!(
        speaker_prefix("Blacksmith: I can mend that."),
        Some(("Blacksmith", "I can mend that."))
    );
    assert_eq!(
        speaker_prefix("Old Man Jenkins: get off my land"),
        Some(("Old Man Jenkins", "get off my land"))
    );
}

#[test]
fn a_section_of_keys_names_what_its_strings_are() {
    let graph = ContentGraph {
        nodes: vec![
            key_node("game.properties", "quest.iron.name", "Iron"),
            key_node("game.properties", "quest.iron.step1", "Find"),
            key_node("game.properties", "quest.iron.step2", "Return"),
        ],
    };
    let inference = infer(&graph);
    let reading = inference
        .reading("game.properties-quest.iron.name")
        .unwrap();
    assert_eq!(reading.context, Some(ContextType::Quest));
    assert!(
        reading.why[0].contains("names a quest"),
        "{:?}",
        reading.why
    );
}

/// Two keys are not a section.
#[test]
fn a_pair_of_keys_is_not_a_section() {
    let graph = ContentGraph {
        nodes: vec![
            key_node("game.properties", "quest.a", "Find"),
            key_node("game.properties", "quest.b", "Return"),
        ],
    };
    assert!(infer(&graph).readings.is_empty());
}

/// Where the group has no name anybody agrees on, what its own members are is the evidence.
#[test]
fn an_unnamed_section_is_read_from_its_own_members() {
    let graph = ContentGraph {
        nodes: vec![
            key_node("g.properties", "zzz.1", "The road east is closed."),
            key_node("g.properties", "zzz.2", "Nobody has passed in weeks."),
            key_node("g.properties", "zzz.3", "The bridge fell in the storm."),
            key_node("g.properties", "zzz.4", "Find"),
        ],
    };
    let inference = infer(&graph);
    let reading = inference.reading("g.properties-zzz.4").unwrap();
    assert_eq!(reading.context, Some(ContextType::Dialogue));
    assert!(
        reading.why[0].contains("mostly dialogue"),
        "{:?}",
        reading.why
    );
}

/// Shape is evidence about the string; neighbourhood is evidence about its surroundings. Where
/// they disagree the string wins, because it is the thing being translated.
#[test]
fn a_string_that_spoke_for_itself_is_not_overruled() {
    let graph = ContentGraph {
        nodes: vec![
            key_node("g.properties", "menu.1", "Start"),
            key_node("g.properties", "menu.2", "Options"),
            key_node("g.properties", "menu.3", "/gfx/icon.png"),
            key_node("g.properties", "menu.4", "Quit"),
        ],
    };
    let inference = infer(&graph);
    // The path was classified technical from its own shape and stays technical.
    assert!(inference
        .reading("g.properties-menu.3")
        .and_then(|r| r.context)
        .is_none());
}

#[test]
fn a_game_with_nothing_to_infer_infers_nothing() {
    let graph = ContentGraph { nodes: Vec::new() };
    let inference = infer(&graph);
    assert!(inference.readings.is_empty());
    assert!(inference.cast.is_empty());
    assert_eq!(
        inference.voice("anything"),
        (Speaker::System, Stance::Neutral)
    );
}
