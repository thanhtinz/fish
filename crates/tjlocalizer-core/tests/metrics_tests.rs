//! Measuring a line in the game's own pixels (§24).
//!
//! The sheets here are proportional on purpose - every letter a different width - because that is
//! the case a character count gets wrong, and the only case where measuring earns its keep.

use tjlocalizer_core::font::metrics::Metrics;
use tjlocalizer_core::font::sheet::{Grid, Image, Sheet};

const CELL: u32 = 12;
const COLUMNS: u32 = 16;

/// A sheet where each letter's width is decided by the caller, so a test can state the widths it
/// is reasoning about instead of deriving them from a picture.
fn sheet_with(width_of: impl Fn(char) -> u32) -> Sheet {
    let characters: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    let rows = (characters.len() as u32).div_ceil(COLUMNS);
    let grid = Grid {
        cell_width: CELL,
        cell_height: CELL,
        columns: COLUMNS,
        rows,
    };
    let mut image = Image::new(COLUMNS * CELL, rows * CELL);

    for (i, c) in characters.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let (ox, oy) = grid.cell_origin(i as u32);
        let width = width_of(*c).clamp(1, CELL);
        for y in 0..5u32 {
            for x in 0..width {
                image.set(ox + x, oy + 3 + y, [220, 200, 120, 255]);
            }
        }
    }
    Sheet::ascii(image, grid)
}

#[test]
fn a_wide_letter_measures_wider_than_a_narrow_one() {
    let sheet = sheet_with(|c| match c {
        'i' => 1,
        'W' => 10,
        _ => 5,
    });
    let metrics = Metrics::of(&sheet);

    assert!(!metrics.monospaced);
    assert_eq!(metrics.width_of('i'), Some(1));
    assert_eq!(metrics.width_of('W'), Some(10));

    // The point of the whole module: same character count, very different width.
    let thin = metrics.measure("iiiii").unwrap();
    let wide = metrics.measure("WWWWW").unwrap();
    assert!(
        wide > thin * 3,
        "five W measured {wide} against {thin} for five i"
    );
}

#[test]
fn a_line_is_its_letters_plus_the_gap_between_them() {
    let sheet = sheet_with(|_| 4);
    let metrics = Metrics::of(&sheet);

    // Four letters at four pixels, with one pixel between each: 16 + 3.
    assert_eq!(metrics.measure("abcd"), Some(19));
    // One letter has no gaps at all.
    assert_eq!(metrics.measure("a"), Some(4));
    assert_eq!(metrics.measure(""), Some(0));
}

/// A sheet drawn on a fixed pitch says so, because there measuring adds nothing that counting
/// characters did not already say, and the layout check stands down rather than repeating it.
#[test]
fn a_fixed_pitch_sheet_is_reported_as_one() {
    let metrics = Metrics::of(&sheet_with(|_| 6));
    assert!(metrics.monospaced);
}

/// A string the sheet cannot draw has no width, and inventing one would produce a second
/// complaint about a string whose real problem is already reported.
#[test]
fn a_letter_the_sheet_lacks_makes_the_line_unmeasurable() {
    let metrics = Metrics::of(&sheet_with(|_| 5));
    // Six letters at five pixels, one space at half a cell, and a pixel between all seven.
    assert_eq!(metrics.measure("Bat dau"), Some(5 * 6 + CELL / 2 + 6));
    assert_eq!(metrics.measure("Bắt đầu"), None);
}

/// A wrapped string is as wide as its widest line, not as wide as all of them end to end.
#[test]
fn a_multi_line_string_measures_its_widest_line() {
    let sheet = sheet_with(|c| if c == 'W' { 10 } else { 2 });
    let metrics = Metrics::of(&sheet);

    let one_line = metrics.measure("WW").unwrap();
    let two_lines = metrics.measure("aaaaaa\nWW").unwrap();
    assert_eq!(two_lines, one_line.max(metrics.measure("aaaaaa").unwrap()));
    assert!(two_lines < metrics.measure("aaaaaaWW").unwrap());
}

mod layout_check {
    use super::*;
    use tjlocalizer_core::graph::{ContentGraph, ContextType, TextNode, TextSource};
    use tjlocalizer_core::jar::Archive;
    use tjlocalizer_core::lang::Language;
    use tjlocalizer_core::translation::TranslationStore;
    use tjlocalizer_core::validate::validate_with_layout;

    fn node(text: &str, context: ContextType) -> TextNode {
        TextNode {
            id: format!("n:{text}"),
            source: TextSource::ClassConstant {
                class: "Game.class".into(),
                utf8_index: 1,
                string_index: 2,
            },
            source_text: text.to_string(),
            source_encoding: None,
            context,
            constraints: Default::default(),
        }
    }

    fn findings(source: &str, target: &str, context: ContextType, monospaced: bool) -> Vec<String> {
        let sheet = sheet_with(move |c| {
            if monospaced {
                5
            } else if c.is_ascii_uppercase() {
                9
            } else {
                3
            }
        });
        let metrics = Metrics::of(&sheet);

        let mut graph = ContentGraph::default();
        graph.nodes.push(node(source, context));
        let mut translations = TranslationStore::default();
        translations.set(format!("n:{source}"), target);

        let archive = Archive::read(
            &std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/data/sample-game.jar"
            ))
            .unwrap(),
        )
        .unwrap();

        validate_with_layout(
            &archive,
            &archive,
            &graph,
            &translations,
            &Language::new("en"),
            &Language::new("vi-VN"),
            None,
            Some(&metrics),
        )
        .findings
        .into_iter()
        .filter(|f| f.check == "layout.width")
        .map(|f| f.detail)
        .collect()
    }

    /// The case this check exists for: a button sized for a short English label.
    #[test]
    fn a_label_that_grew_far_wider_than_its_original_is_flagged() {
        let found = findings("OK", "KHONG DONG Y DAU", ContextType::Ui, false);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("pixels wide"), "{}", found[0]);
    }

    /// A longer string that draws no wider is not a layout problem, whatever its character count
    /// says. This is the whole reason for measuring rather than counting.
    #[test]
    fn more_letters_in_narrower_pixels_is_not_flagged() {
        assert!(findings("OKOK", "iiiiii", ContextType::Ui, false).is_empty());
    }

    /// Dialogue wraps. A long line there is a line, not a bug, and warning about it would teach
    /// people to ignore the warning that matters.
    #[test]
    fn dialogue_is_not_measured_against_a_button() {
        assert!(findings("OK", "KHONG DONG Y DAU", ContextType::Dialogue, false).is_empty());
    }

    /// On a fixed-pitch sheet this measurement is the character count in other units, and the
    /// length check already made that point.
    #[test]
    fn a_fixed_pitch_game_gets_no_pixel_warning() {
        assert!(findings("OK", "KHONG DONG Y DAU", ContextType::Ui, true).is_empty());
    }
}
