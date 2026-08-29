//! Register: the part that decides whether a line sounds like this game or like a machine.

use tjlocalizer_core::lang::Language;
use tjlocalizer_core::register::{builtin, builtin_profiles, profiles_for, Speaker, Stance};

/// The problem this module exists for. Vietnamese has no neutral second person, so "Are you
/// sure?" has several right answers and choosing between them is not a vocabulary question.
#[test]
fn each_register_names_a_different_second_person() {
    let wuxia = builtin("natural-dialogue").unwrap();
    let modern = builtin("modern").unwrap();
    let formal = builtin("formal").unwrap();

    assert_eq!(
        wuxia
            .pronouns(Speaker::Npc, Stance::Neutral)
            .second_singular,
        "ngươi"
    );
    assert_eq!(
        modern
            .pronouns(Speaker::Npc, Stance::Neutral)
            .second_singular,
        "bạn"
    );
    assert_eq!(
        formal
            .pronouns(Speaker::System, Stance::Neutral)
            .second_singular,
        "quý khách"
    );
}

/// Interface text takes no pronoun at all. Inserting one is a common way a translated menu reads
/// as translated.
#[test]
fn interface_text_gets_no_pronoun() {
    let wuxia = builtin("natural-dialogue").unwrap();
    let terse = builtin("terse-ui").unwrap();

    assert_eq!(
        wuxia
            .pronouns(Speaker::System, Stance::Neutral)
            .second_singular,
        ""
    );
    assert_eq!(
        terse
            .pronouns(Speaker::System, Stance::Neutral)
            .second_singular,
        ""
    );
}

#[test]
fn stance_changes_the_voice() {
    let wuxia = builtin("natural-dialogue").unwrap();
    assert_eq!(
        wuxia
            .pronouns(Speaker::Npc, Stance::Deferential)
            .first_singular,
        "tại hạ"
    );
    assert_eq!(
        wuxia.pronouns(Speaker::Npc, Stance::Neutral).first_singular,
        "ta"
    );
}

/// An unknown combination falls back rather than returning nothing, so a line always has a voice.
#[test]
fn an_unmodelled_combination_falls_back_to_the_neutral_voice() {
    let modern = builtin("modern").unwrap();
    let pronouns = modern.pronouns(Speaker::Narrator, Stance::Hostile);
    assert!(!pronouns.third_male.is_empty());
}

/// A modern pronoun in a wuxia game is the single most common register break.
#[test]
fn wording_that_breaks_the_register_is_reported() {
    let wuxia = builtin("natural-dialogue").unwrap();
    let issues = wuxia.check("Bạn có chắc không?");
    assert!(issues.iter().any(|i| i.code == "register"), "{issues:?}");

    let clean = wuxia.check("Ngươi chắc chứ?");
    assert!(clean.iter().all(|i| i.code != "register"), "{clean:?}");
}

/// Vietnamese words are short and sit inside one another; a substring test would flag half the
/// text. "ta" is in "hoàn tất" and "tay", and neither is the pronoun.
#[test]
fn a_pronoun_inside_another_word_is_not_a_register_break() {
    let modern = builtin("modern").unwrap();
    for text in ["Hoàn tất nhiệm vụ", "Trang bị vào tay phải", "Bạc và vàng"] {
        let issues = modern.check(text);
        assert!(
            issues.iter().all(|i| i.code != "register"),
            "{text:?} -> {issues:?}"
        );
    }
}

#[test]
fn the_wuxia_register_prefers_its_own_vocabulary() {
    let wuxia = builtin("natural-dialogue").unwrap();
    let issues = wuxia.check("Chọn vũ khí");
    assert!(
        issues.iter().any(|i| i.code == "wording"),
        "binh khí is the wuxia reading: {issues:?}"
    );
}

#[test]
fn every_profile_belongs_to_a_language_and_is_findable_by_it() {
    for profile in builtin_profiles() {
        assert!(!profile.id.is_empty());
        assert!(
            profiles_for(&profile.language)
                .iter()
                .any(|p| p.id == profile.id),
            "{} is not listed for {}",
            profile.id,
            profile.language
        );
    }
    assert!(profiles_for(&Language::new("vi-VN")).len() >= 4);
}
