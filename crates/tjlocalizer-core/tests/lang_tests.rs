//! Language identity and the rules that follow from it.

use tjlocalizer_core::lang::{known_languages, Language, Script};
use tjlocalizer_core::quality;
use tjlocalizer_core::translation::Issue;

#[test]
fn a_tag_is_read_by_its_parts() {
    let vi = Language::new("vi-VN");
    assert_eq!(vi.base(), "vi");
    assert_eq!(vi.region().as_deref(), Some("VN"));
    assert_eq!(vi.script_subtag(), None);

    let zh = Language::new("zh-Hans");
    assert_eq!(zh.base(), "zh");
    assert_eq!(zh.script_subtag().as_deref(), Some("Hans"));
    // Hans is four letters, so it is a script and not a region.
    assert_eq!(zh.region(), None);
}

#[test]
fn a_region_does_not_make_a_different_language() {
    assert!(Language::new("vi").same_language_as(&Language::new("vi-VN")));
    assert!(Language::new("zh-Hans").same_language_as(&Language::new("zh-TW")));
    assert!(!Language::new("vi").same_language_as(&Language::new("th")));
}

#[test]
fn scripts_are_derived_from_the_tag() {
    assert_eq!(Language::new("vi-VN").script(), Script::Latin);
    assert_eq!(Language::new("zh-Hans").script(), Script::Han);
    assert_eq!(Language::new("ja").script(), Script::Japanese);
    assert_eq!(Language::new("ko").script(), Script::Korean);
    assert_eq!(Language::new("ru").script(), Script::Cyrillic);
    assert_eq!(Language::new("th").script(), Script::Thai);
}

#[test]
fn scripts_that_run_words_together_are_known_to() {
    assert!(Language::new("vi").script().uses_spaces_between_words());
    assert!(!Language::new("zh").script().uses_spaces_between_words());
    assert!(!Language::new("th").script().uses_spaces_between_words());
    assert!(!Language::new("ja").script().uses_spaces_between_words());
}

/// The length check has to account for how densely each script writes, or it is useless in one
/// direction and deafening in the other.
#[test]
fn a_chinese_translation_of_an_english_label_is_not_flagged_as_too_long() {
    let en = Language::new("en");
    let zh = Language::new("zh-Hans");
    let issues = quality::check(
        "Start Game and Continue Your Adventure",
        "开始游戏并继续冒险",
        &[],
        &en,
        &zh,
    );
    assert!(
        !issues.iter().any(|i: &Issue| i.code == "length"),
        "{issues:?}"
    );
}

#[test]
fn a_translation_several_times_its_source_is_flagged() {
    let en = Language::new("en");
    let vi = Language::new("vi-VN");
    let issues = quality::check(
        "Quit",
        "Thoát khỏi trò chơi và quay lại màn hình chính của thiết bị ngay lập tức",
        &[],
        &en,
        &vi,
    );
    assert!(issues.iter().any(|i| i.code == "length"), "{issues:?}");
}

/// Vietnamese sets no space before a comma; Chinese punctuation is full-width and carries its
/// own. Applying the Vietnamese rule to Chinese would rewrite correct text.
#[test]
fn normalisation_follows_the_target_language() {
    assert_eq!(
        quality::normalize("Xin chào , thế giới", &Language::new("vi-VN")),
        "Xin chào, thế giới"
    );
    let chinese = "你好 ， 世界";
    assert_eq!(
        quality::normalize(chinese, &Language::new("zh-Hans")),
        chinese,
        "full-width punctuation must be left alone"
    );
}

/// Text left in the source script is text nobody translated, and it passes every other check.
#[test]
fn untranslated_source_script_is_caught() {
    let issues = quality::check(
        "装备强化",
        "Cường hoá 装备",
        &[],
        &Language::new("zh"),
        &Language::new("vi-VN"),
    );
    assert!(issues.iter().any(|i| i.code == "script"), "{issues:?}");
}

#[test]
fn the_shipped_languages_all_have_a_usable_profile() {
    for language in known_languages() {
        let profile = language.profile();
        assert_eq!(profile.language, language);
        assert!(profile.expansion_limit > 0.0);
        // A budget must leave room for a translation at all.
        assert!(profile.length_budget(10, &Language::new("en")) >= 4);
    }
}
