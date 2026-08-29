//! The Vietnamese alphabet, and what a font is missing.

use tjlocalizer_core::font::{
    report, vietnamese_compositions, vietnamese_required, Coverage, Tone, VowelMark,
};

/// The specification lists 134 letters. Building them from the twelve vowels rather than writing
/// the list out is the point: a hand-typed table has a typo in it that nobody finds until a game
/// ships with one letter blank.
#[test]
fn the_alphabet_is_exactly_what_the_specification_lists() {
    let required = vietnamese_required();
    assert_eq!(required.len(), 134, "expected 134 letters beyond ASCII");

    // Spot checks across every shape: modified vowels, each tone, both cases, and đ.
    for c in "ăâêôơưđĂÂÊÔƠƯĐ".chars() {
        assert!(required.contains(&c), "{c} is missing");
    }
    for c in "áàảãạắằẳẵặấầẩẫậ".chars() {
        assert!(required.contains(&c), "{c} is missing");
    }
    for c in "ếềểễệứừửữựýỳỷỹỵ".chars() {
        assert!(required.contains(&c), "{c} is missing");
    }
    for c in "ỐỒỔỖỘỚỜỞỠỢ".chars() {
        assert!(required.contains(&c), "{c} is missing");
    }
    // ASCII vowels are not "required beyond ASCII".
    for c in "aeiouyAEIOUY".chars() {
        assert!(!required.contains(&c), "{c} should not be listed");
    }
}

#[test]
fn every_letter_is_built_from_an_ascii_base() {
    for composition in vietnamese_compositions() {
        assert!(
            composition.base.is_ascii_alphabetic(),
            "{} is built from {:?}, which is not ASCII",
            composition.composed,
            composition.base
        );
        assert_ne!(composition.composed, composition.base);
        // A letter with neither a modification nor a tone would be the base itself.
        assert!(
            composition.vowel_mark.is_some() || composition.tone.is_some(),
            "{} has nothing to draw",
            composition.composed
        );
    }
}

#[test]
fn case_is_carried_through_the_composition() {
    let compositions = vietnamese_compositions();
    let upper = compositions
        .iter()
        .find(|c| c.composed == 'Ế')
        .expect("Ế must be composable");
    assert_eq!(upper.base, 'E');
    assert_eq!(upper.vowel_mark, Some(VowelMark::Circumflex));
    assert_eq!(upper.tone, Some(Tone::Acute));

    let lower = compositions.iter().find(|c| c.composed == 'ế').unwrap();
    assert_eq!(lower.base, 'e');
}

/// A modified vowel is a different letter, not the same letter said differently, so it needs a
/// glyph even in text with no tones.
#[test]
fn a_modified_vowel_with_no_tone_is_still_required() {
    let compositions = vietnamese_compositions();
    let horn = compositions.iter().find(|c| c.composed == 'ơ').unwrap();
    assert_eq!(horn.base, 'o');
    assert_eq!(horn.vowel_mark, Some(VowelMark::Horn));
    assert_eq!(horn.tone, None);
}

/// đ takes no tone; a table that gave it one would produce characters that do not exist.
#[test]
fn the_stroked_d_takes_no_tone() {
    let toned: Vec<char> = vietnamese_compositions()
        .iter()
        .filter(|c| c.base.eq_ignore_ascii_case(&'d') && c.tone.is_some())
        .map(|c| c.composed)
        .collect();
    assert!(toned.is_empty(), "{toned:?}");
}

#[test]
fn the_dot_is_the_only_mark_drawn_below() {
    for tone in Tone::all() {
        assert_eq!(tone.is_below(), tone == Tone::DotBelow, "{tone:?}");
    }
}

/// The case this module exists for: a font drawn for a game covers ASCII and nothing else, so
/// every one of the 134 letters comes out blank.
#[test]
fn an_ascii_font_is_missing_the_whole_alphabet() {
    let coverage = Coverage::ascii("game font sheet");
    assert_eq!(coverage.missing_for_vietnamese().len(), 134);
    // But it has every base letter, so every one of them can be built.
    assert_eq!(coverage.composable().len(), 134);
}

#[test]
fn a_font_without_its_base_letters_cannot_have_them_composed() {
    // A sheet holding only digits and punctuation - a score font, say.
    let coverage = Coverage::new("0123456789:/%".chars(), "score font");
    assert_eq!(coverage.composable().len(), 0);
    assert_eq!(coverage.missing_for_vietnamese().len(), 134);
}

/// Whitespace is laid out, not drawn, so a font is not missing a glyph for it.
#[test]
fn whitespace_is_never_reported_as_missing() {
    let coverage = Coverage::ascii("sheet");
    assert!(coverage.missing_in("Start Game\n\t").is_empty());
}

/// In the order they appear in the text, not sorted: a translator reading a line wants to find
/// the first problem first.
#[test]
fn missing_characters_are_reported_once_each_in_the_order_they_appear() {
    let coverage = Coverage::ascii("sheet");
    assert_eq!(coverage.missing_in("Sinh lực: ế ế ự"), vec!['ự', 'ế']);
    assert_eq!(coverage.missing_in("ế ự ế"), vec!['ế', 'ự']);
}

/// The report a translator needs: which strings will show blanks, and what is missing.
#[test]
fn the_report_names_what_the_translations_actually_use() {
    let coverage = Coverage::ascii("game font sheet");
    let strings = vec![
        "Bắt đầu trò chơi",
        "Thoat", // no diacritics at all
        "Sinh lực: %d / %d",
    ];
    let report = report(&coverage, strings);

    assert_eq!(report.affected_strings, 2, "the mark-free string is fine");
    assert!(report.missing_used.contains(&'ắ'));
    assert!(report.missing_used.contains(&'ự'));
    assert!(!report.missing_used.contains(&'T'));
    assert_eq!(report.missing_required.len(), 134);
    assert_eq!(report.composable_count, 134);
}

#[test]
fn a_font_that_already_covers_vietnamese_reports_nothing_missing() {
    let mut covered: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    covered.extend(vietnamese_required());
    let coverage = Coverage::new(covered, "device font");

    assert!(coverage.missing_for_vietnamese().is_empty());
    assert!(coverage.composable().is_empty());
    let report = report(&coverage, vec!["Bắt đầu trò chơi"]);
    assert_eq!(report.affected_strings, 0);
    assert!(report.missing_used.is_empty());
}
