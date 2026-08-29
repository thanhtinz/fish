//! The dictionary: term resolution, domains, and the glosses built on top.

use tjlocalizer_core::dictionary::{Domain, Segment};
use tjlocalizer_core::dictionary_data;
use tjlocalizer_core::lang::Language;
use tjlocalizer_core::register;
use tjlocalizer_core::translate::{propose, Completeness, DictionaryProvider, Request};
use tjlocalizer_core::translation::{Glossary, GlossaryEntry, TranslationMemory};

fn request(text: &str, from: &str, to: &str, context: &str) -> Request {
    Request {
        source_text: text.into(),
        from: Language::new(from),
        to: Language::new(to),
        context: context.into(),
        placeholders: tjlocalizer_core::graph::find_placeholders(text),
        speaker: Default::default(),
        stance: Default::default(),
    }
}

#[test]
fn every_shipped_pack_parses() {
    let errors = dictionary_data::builtin_errors();
    assert!(errors.is_empty(), "{errors:?}");
    let dictionary = dictionary_data::builtin();
    assert!(dictionary.entry_count() > 400);
}

#[test]
fn the_directions_the_user_asked_for_are_covered() {
    let dictionary = dictionary_data::builtin();
    let directions = dictionary.directions();
    for (from, to) in [
        ("zh", "vi"),
        ("en", "vi"),
        ("ja", "vi"),
        ("ko", "vi"),
        ("ru", "vi"),
        ("en", "zh"),
        ("en", "th"),
        ("en", "id"),
    ] {
        assert!(
            directions
                .iter()
                .any(|(f, t)| f.base() == from && t.base() == to),
            "no {from} to {to} pack"
        );
    }
}

/// The reading that makes this a game dictionary rather than a general one. A general dictionary
/// gives "thiết bị" for 装备, which is hardware, and every J2ME translation that used one says so.
#[test]
fn game_readings_beat_general_ones() {
    let dictionary = dictionary_data::builtin();
    let zh = Language::new("zh");
    let vi = Language::new("vi-VN");

    let reading = dictionary.lookup("装备", &zh, &vi, "ui").unwrap();
    assert_eq!(reading.target, "trang bị");
    assert_eq!(reading.domain, Domain::Item);

    let guild = dictionary
        .lookup("Guild", &Language::new("en"), &vi, "ui")
        .unwrap();
    assert_eq!(guild.target, "bang hội");
}

/// Longest match first, or the shorter term matches inside the longer one and leaves a stray
/// character behind.
#[test]
fn the_longest_term_wins() {
    let dictionary = dictionary_data::builtin();
    let segments = dictionary.segment("攻击力", &Language::new("zh"), &Language::new("vi"), "ui");
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        Segment::Term { reading, .. } => assert_eq!(reading.target, "lực tấn công"),
        other => panic!("expected one term, got {other:?}"),
    }
}

/// In a spaced script a term must be a whole word: "Use" is a term, the "use" inside "Because"
/// is not.
#[test]
fn a_term_is_not_matched_inside_a_longer_word() {
    let dictionary = dictionary_data::builtin();
    let segments = dictionary.segment("Because", &Language::new("en"), &Language::new("vi"), "ui");
    assert!(
        segments.iter().all(|s| !matches!(s, Segment::Term { .. })),
        "{segments:?}"
    );
}

#[test]
fn placeholders_are_carried_through_untouched() {
    let dictionary = dictionary_data::builtin();
    let glossary = Glossary::default();
    let provider = DictionaryProvider::new(&dictionary, &glossary);
    let memory = TranslationMemory::default();

    let proposal = propose(
        &request("HP: %d / %d", "en", "vi-VN", "format"),
        &memory,
        &[&provider],
    )
    .expect("a fully resolvable string should be glossed");

    assert!(proposal.target_text.contains("sinh lực"));
    assert_eq!(
        proposal.target_text.matches("%d").count(),
        2,
        "both placeholders must survive: {:?}",
        proposal.target_text
    );
    assert_eq!(proposal.completeness, Completeness::Complete);
}

/// The failure this rule exists for. "Dragon Quest Online" is a title; the dictionary knows only
/// "Quest", and substituting it gives "Dragon nhiệm vụ Online" - which looks like a translation,
/// invites a tired reviewer to accept it, and is worse than proposing nothing.
#[test]
fn a_mostly_unresolved_string_is_not_glossed_at_all() {
    let dictionary = dictionary_data::builtin();
    let glossary = Glossary::default();
    let provider = DictionaryProvider::new(&dictionary, &glossary);
    let memory = TranslationMemory::default();

    let proposal = propose(
        &request("Dragon Quest Online", "en", "vi-VN", "story"),
        &memory,
        &[&provider],
    );
    assert!(proposal.is_none(), "{proposal:?}");
}

/// However complete, a dictionary gloss is never approvable on its own.
#[test]
fn no_gloss_is_ever_auto_approvable() {
    let dictionary = dictionary_data::builtin();
    let glossary = Glossary::default();
    let provider = DictionaryProvider::new(&dictionary, &glossary);
    let memory = TranslationMemory::default();

    for text in ["Start Game", "Quit", "HP: %d / %d"] {
        let proposal = propose(&request(text, "en", "vi-VN", "ui"), &memory, &[&provider]).unwrap();
        assert_eq!(proposal.completeness, Completeness::Complete);
        assert!(!proposal.is_approvable(), "{text}");
        assert!(proposal.confidence <= 0.8);
    }
}

/// The project's own glossary is a decision; the dictionary is a general reading.
#[test]
fn the_glossary_overrides_the_dictionary() {
    let dictionary = dictionary_data::builtin();
    let glossary = Glossary {
        entries: vec![GlossaryEntry {
            source: "Quit".into(),
            target: "Rời khỏi".into(),
            locked: true,
            note: String::new(),
        }],
    };
    let provider = DictionaryProvider::new(&dictionary, &glossary);
    let memory = TranslationMemory::default();

    let proposal = propose(&request("Quit", "en", "vi-VN", "ui"), &memory, &[&provider]).unwrap();
    assert_eq!(proposal.target_text, "Rời khỏi");
}

/// Wording a person already approved beats anything an engine produces for the same string.
#[test]
fn the_memory_is_consulted_before_any_engine() {
    let dictionary = dictionary_data::builtin();
    let glossary = Glossary::default();
    let provider = DictionaryProvider::new(&dictionary, &glossary);
    let mut memory = TranslationMemory::default();
    memory.remember("Quit", "Thoát game", None);

    let proposal = propose(&request("Quit", "en", "vi-VN", "ui"), &memory, &[&provider]).unwrap();
    assert_eq!(proposal.engine, "memory");
    assert_eq!(proposal.target_text, "Thoát game");
    assert_eq!(proposal.confidence, 1.0);
}

#[test]
fn a_direction_with_no_pack_proposes_nothing() {
    let dictionary = dictionary_data::builtin();
    let glossary = Glossary::default();
    let provider = DictionaryProvider::new(&dictionary, &glossary);
    let memory = TranslationMemory::default();

    let proposal = propose(&request("Quit", "km", "lo", "ui"), &memory, &[&provider]);
    assert!(proposal.is_none());
}

/// The register carried by a proposal is reported, and wording that breaks it is noted.
#[test]
fn a_gloss_reports_the_register_it_was_made_under() {
    let dictionary = dictionary_data::builtin();
    let glossary = Glossary::default();
    let style = register::builtin("natural-dialogue").unwrap();
    let provider = DictionaryProvider::new(&dictionary, &glossary).with_style(&style);
    let memory = TranslationMemory::default();

    let proposal = propose(
        &request("Start Game", "en", "vi-VN", "ui"),
        &memory,
        &[&provider],
    )
    .unwrap();
    assert_eq!(proposal.register.as_deref(), Some("natural-dialogue"));
}
