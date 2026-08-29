//! Making a label fit (§24).
//!
//! The check that says a translation is too wide is only half a tool. These are the offers that
//! follow it - and the tests are mostly about what is *not* offered, because a suggestion list
//! that rewrites a translator's wording is worse than an empty one.

use tjlocalizer_core::dictionary::{Dictionary, Domain, Entry, Pack};
use tjlocalizer_core::font::metrics::Metrics;
use tjlocalizer_core::font::sheet::{Grid, Image, Sheet};
use tjlocalizer_core::lang::Language;
use tjlocalizer_core::shorten;

fn en() -> Language {
    Language::new("en")
}

fn vi() -> Language {
    Language::new("vi-VN")
}

fn entry(source: &str, target: &str, priority: i32, note: &str) -> Entry {
    Entry {
        source: source.into(),
        target: target.into(),
        domain: Domain::Ui,
        priority,
        note: note.into(),
    }
}

fn dictionary(entries: Vec<Entry>) -> Dictionary {
    let mut dictionary = Dictionary::default();
    dictionary.add(Pack {
        from: en(),
        to: vi(),
        source_note: "test".into(),
        entries,
    });
    dictionary
}

/// A proportional sheet covering the letters these tests use, so widths are real.
fn metrics() -> Metrics {
    let characters: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    let (cell, columns) = (12u32, 16u32);
    let rows = (characters.len() as u32).div_ceil(columns);
    let grid = Grid {
        cell_width: cell,
        cell_height: cell,
        columns,
        rows,
    };
    let mut image = Image::new(columns * cell, rows * cell);
    for (i, c) in characters.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let (ox, oy) = grid.cell_origin(i as u32);
        let width = if c.is_ascii_uppercase() { 6 } else { 3 };
        for y in 0..5u32 {
            for x in 0..width {
                image.set(ox + x, oy + 4 + y, [230, 230, 230, 255]);
            }
        }
    }
    Metrics::of(&Sheet::ascii(image, grid))
}

fn offers(source: &str, current: &str, dictionary: &Dictionary) -> Vec<shorten::Alternative> {
    let metrics = metrics();
    shorten::alternatives(
        source,
        current,
        dictionary,
        &en(),
        &vi(),
        "ui",
        Some(&metrics),
    )
}

/// A dictionary carrying two words for one term is carrying a choice, and a button sometimes
/// needs the shorter one.
#[test]
fn a_second_reading_is_offered_when_it_is_narrower() {
    let dictionary = dictionary(vec![
        entry("Settings", "cai dat", 10, ""),
        entry("Settings", "tuy chinh he thong", 0, "dai hon"),
    ]);
    let found = offers("Settings", "tuy chinh he thong", &dictionary);

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].text, "cai dat");
    assert!(found[0].width.unwrap() > 0);
    assert!(
        found[0].why.contains("cai dat"),
        "the reason does not name the change: {}",
        found[0].why
    );
}

/// The longer reading is never offered as a way to make something fit.
#[test]
fn a_longer_reading_is_not_offered() {
    let dictionary = dictionary(vec![
        entry("Settings", "cai dat", 10, ""),
        entry("Settings", "tuy chinh he thong", 0, ""),
    ]);
    assert!(offers("Settings", "cai dat", &dictionary).is_empty());
}

/// Substituting into a line that says something else would rewrite a translator's own wording
/// behind their back, which is the one thing this must never do.
#[test]
fn a_reading_the_translation_did_not_use_is_left_alone() {
    let dictionary = dictionary(vec![
        entry("Settings", "cai dat", 10, ""),
        entry("Settings", "tuy chinh", 0, ""),
    ]);
    // A translator wrote something of their own. Nothing here matches it, so nothing is proposed.
    assert!(offers("Settings", "thiet lap rieng cua toi", &dictionary).is_empty());
}

/// Vietnamese interface text takes no pronoun: a button says "Thoat", not "Ban thoat". The
/// register profile already says so, so the offer comes from there rather than from an opinion.
#[test]
fn an_interface_pronoun_is_offered_for_removal() {
    let dictionary = dictionary(vec![]);
    let found = offers("Exit", "bạn thoát", &dictionary);

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].text, "thoát");
    assert!(found[0].why.contains("bạn"), "{}", found[0].why);
}

/// Fewer characters is not narrower. Measuring in the game's own pixels is the entire point, and
/// an offer that is longer on screen must not be presented as a saving.
#[test]
fn an_alternative_with_fewer_letters_but_more_pixels_is_not_offered() {
    // Six capitals against seven lower-case letters: fewer characters, far more pixels.
    let dictionary = dictionary(vec![
        entry("Menu", "aaaaaaa", 10, ""),
        entry("Menu", "BBBBBB", 0, ""),
    ]);
    let found = offers("Menu", "aaaaaaa", &dictionary);
    assert!(
        found.is_empty(),
        "a wider alternative was offered as shorter: {found:?}"
    );
}

/// A project with no declared font still gets help, on the worse question, rather than nothing.
#[test]
fn without_a_sheet_the_offers_fall_back_to_counting_characters() {
    let dictionary = dictionary(vec![
        entry("Settings", "cai dat", 10, ""),
        entry("Settings", "tuy chinh he thong", 0, ""),
    ]);
    let found = shorten::alternatives(
        "Settings",
        "tuy chinh he thong",
        &dictionary,
        &en(),
        &vi(),
        "ui",
        None,
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].text, "cai dat");
    assert_eq!(found[0].width, None, "a width was invented with no font");
}

/// Narrowest first: a list somebody reads top-down should start with the one most likely to fit.
#[test]
fn offers_are_ordered_by_what_they_measure() {
    let dictionary = dictionary(vec![
        entry("Settings", "thiet lap he thong", 10, ""),
        entry("Settings", "cai", 0, ""),
        entry("Settings", "cai dat", 0, ""),
    ]);
    let found = offers("Settings", "thiet lap he thong", &dictionary);
    let widths: Vec<u32> = found.iter().filter_map(|a| a.width).collect();
    assert_eq!(widths.len(), 2, "{found:?}");
    assert!(widths[0] < widths[1], "{widths:?}");
}

/// A plain substring replace would cut a word in half and offer the result as an improvement.
#[test]
fn a_term_inside_another_word_is_not_replaced() {
    let dictionary = dictionary(vec![
        entry("Start", "bat dau", 10, ""),
        entry("Start", "mo", 0, ""),
    ]);
    // "bat" appears inside "bat dau", and only the whole term may be substituted.
    let found = offers("Start", "bat dau", &dictionary);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].text, "mo");
}

/// The case that made this feature offer nothing at all on real data.
///
/// A dictionary keeps its readings in lower case; a label in a game is capitalised. An exact
/// match finds neither the reading the translation used nor anything to replace, so the list came
/// back empty on every row - which reads as "no shorter way exists" rather than as a bug.
#[test]
fn a_capitalised_label_still_matches_its_lower_case_reading() {
    let dictionary = dictionary(vec![
        entry("Start Game", "bat dau tro choi", 5, ""),
        entry("Start Game", "bat dau", 0, "ngan hon"),
    ]);
    let found = offers("Start Game", "Bat dau tro choi", &dictionary);

    assert_eq!(found.len(), 1, "{found:?}");
    // And the replacement keeps the capital the label had.
    assert_eq!(found[0].text, "Bat dau");
}

/// Dropping a pronoun has to work on the capitalised form too, for the same reason.
#[test]
fn a_capitalised_pronoun_is_still_dropped() {
    let found = offers("Exit", "Bạn thoát", &dictionary(vec![]));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].text, "thoát");
}
