//! Diacritics lifted from a real typeface, and the rule that keeps them safe.
//!
//! These need a font file. One is looked for among the system fonts rather than committed: a
//! font is somebody's work under somebody's licence, and a localization tool has no business
//! carrying one around. When none is found the tests say so and pass, because a missing system
//! font is not a defect in this crate.

use tjlocalizer_core::font::outline::MarkSource;
use tjlocalizer_core::font::sheet::{extend_with_marks, Grid, Image, Sheet};
use tjlocalizer_core::font::{vietnamese_compositions, Tone, VowelMark};

/// A font on this machine that can draw Vietnamese, if there is one.
fn system_font() -> Option<MarkSource> {
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
        "C:/Windows/Fonts/arial.ttf",
    ];
    for path in CANDIDATES {
        let path = std::path::Path::new(path);
        if !path.exists() {
            continue;
        }
        if let Ok(source) = MarkSource::from_path(path) {
            if source.covers_vietnamese() {
                return Some(source);
            }
        }
    }
    None
}

fn sheet(cell: u32, ink_height: u32, padding_top: u32) -> Sheet {
    let characters: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    let columns = 16u32;
    let rows = (characters.len() as u32).div_ceil(columns);
    let mut image = Image::new(columns * cell, rows * cell);
    let grid = Grid {
        cell_width: cell,
        cell_height: cell,
        columns,
        rows,
    };
    for (i, c) in characters.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let (ox, oy) = grid.cell_origin(i as u32);
        let seed = (*c as u32).wrapping_mul(2_654_435_761);
        let width = ink_height.min(6);
        for y in 0..ink_height {
            for x in 0..width {
                let edge = x == 0 || x + 1 == width || y == 0 || y + 1 == ink_height;
                let inked = edge || (seed >> ((y * 4 + x) % 24)) & 1 == 1;
                if inked {
                    image.set(ox + 2 + x, oy + padding_top + y, [255, 255, 255, 255]);
                }
            }
        }
    }
    Sheet::ascii(image, grid)
}

fn clashes(sheet: &Sheet, added: &[char]) -> Vec<(char, char)> {
    let mut seen: std::collections::HashMap<Vec<[u8; 4]>, char> = Default::default();
    let mut out = Vec::new();
    for c in added {
        let index = sheet.index_of(*c).unwrap();
        let (ox, oy) = sheet.grid.cell_origin(index);
        let mut pixels = Vec::new();
        for y in 0..sheet.grid.cell_height {
            for x in 0..sheet.grid.cell_width {
                pixels.push(sheet.image.get(ox + x, oy + y));
            }
        }
        if let Some(other) = seen.insert(pixels, *c) {
            out.push((*c, other));
        }
    }
    out
}

#[test]
fn a_typeface_is_read_and_its_vietnamese_coverage_reported() {
    let Some(source) = system_font() else {
        eprintln!("no system font covering Vietnamese; skipping");
        return;
    };
    assert!(source.has('ế'));
    assert!(source.has('ự'));
    assert!(source.covers_vietnamese());
}

/// The mark is the difference between the composed letter and its base, so it must not contain
/// the base - otherwise stamping it would draw a second letter on top of the first.
#[test]
fn a_mark_is_only_what_the_composed_letter_adds() {
    let Some(source) = system_font() else {
        return;
    };
    let compositions = vietnamese_compositions();
    let acute = compositions.iter().find(|c| c.composed == 'á').unwrap();

    let mark = source.mark_for(acute, 20).expect("á minus a is a mark");
    assert!(!mark.is_empty());
    // An acute sits above the letter, so the offset from the base's top is negative.
    assert!(
        mark.dy < 0,
        "the mark should be above the letter, dy={}",
        mark.dy
    );
    // And it is small: a mark that came out the size of the letter means the subtraction failed.
    assert!(
        mark.height < 20,
        "the mark is {} tall against a 20 pixel letter",
        mark.height
    );
}

/// A dot below is the one mark that goes under, and its offset has to say so.
#[test]
fn a_dot_below_is_placed_under_the_letter() {
    let Some(source) = system_font() else {
        return;
    };
    let compositions = vietnamese_compositions();
    let dot = compositions
        .iter()
        .find(|c| c.composed == 'ạ' && c.tone == Some(Tone::DotBelow))
        .unwrap();

    let mark = source.mark_for(dot, 20).expect("ạ minus a is a mark");
    assert!(
        mark.dy > 0,
        "a dot below must sit under the letter, dy={}",
        mark.dy
    );
}

#[test]
fn a_letter_the_typeface_lacks_yields_no_mark() {
    let Some(source) = system_font() else {
        return;
    };
    let compositions = vietnamese_compositions();
    let any = compositions.first().unwrap();
    // Zero height is not a size anything can be scaled to.
    assert!(source.mark_for(any, 0).is_none());
}

/// The rule that makes borrowing marks safe at all.
///
/// A typeface's diacritics are drawn for reading sizes. Rasterised into a twelve-pixel cell they
/// thin out until a grave and an acute are the same two pixels - measured at 55 identical pairs
/// out of 134 on a real font, which would put "bà" and "bá" on screen as the same word. So a
/// borrowed mark is kept only where the letter stays unlike every other.
#[test]
fn no_two_letters_are_identical_at_any_size_a_game_uses() {
    let Some(source) = system_font() else {
        return;
    };
    let compositions = vietnamese_compositions();

    for (cell, ink, padding) in [(12u32, 5u32, 5u32), (16, 7, 7), (24, 11, 11), (32, 15, 14)] {
        let base = sheet(cell, ink, padding);
        let (extended, report) = extend_with_marks(&base, &compositions, Some(&source)).unwrap();

        assert_eq!(report.added.len(), 134, "at {cell}px: {:?}", report.skipped);
        assert!(
            clashes(&extended, &report.added).is_empty(),
            "at {cell}px: {:?}",
            clashes(&extended, &report.added)
        );
    }
}

/// Which marks came from the typeface is reported, because it is the difference between "this
/// font was used" and "this font was asked and mostly declined".
#[test]
fn the_report_says_how_many_marks_the_typeface_supplied() {
    let Some(source) = system_font() else {
        return;
    };
    let compositions = vietnamese_compositions();

    let small = extend_with_marks(&sheet(12, 5, 5), &compositions, Some(&source))
        .unwrap()
        .1;
    let large = extend_with_marks(&sheet(32, 15, 14), &compositions, Some(&source))
        .unwrap()
        .1;

    assert!(
        large.from_typeface > small.from_typeface,
        "a typeface should serve a large cell better than a small one: {} against {}",
        large.from_typeface,
        small.from_typeface
    );
    assert!(small.typeface.is_some());
}

/// With no typeface the result must be exactly what it always was.
#[test]
fn passing_no_typeface_is_the_same_as_before() {
    let base = sheet(12, 5, 5);
    let compositions = vietnamese_compositions();

    let (a, ra) = extend_with_marks(&base, &compositions, None).unwrap();
    let (b, rb) = tjlocalizer_core::font::sheet::extend(&base, &compositions).unwrap();

    assert_eq!(ra.added, rb.added);
    assert_eq!(ra.from_typeface, 0);
    assert_eq!(rb.typeface, None);
    assert_eq!(a.image.pixels, b.image.pixels);
}

/// The base letter still has to survive, whichever mark was used.
#[test]
fn a_borrowed_mark_does_not_disturb_the_game_s_letter() {
    let Some(source) = system_font() else {
        return;
    };
    let base = sheet(24, 11, 11);
    let (extended, _) =
        extend_with_marks(&base, &vietnamese_compositions(), Some(&source)).unwrap();

    let bounds = base.ink_bounds('e').unwrap();
    let (bx, by) = base.grid.cell_origin(bounds.cell);
    for composed in ['é', 'ê', 'ế'] {
        let index = extended.index_of(composed).unwrap();
        let (cx, cy) = extended.grid.cell_origin(index);
        for y in bounds.y..bounds.y + bounds.height {
            for x in bounds.x..bounds.x + bounds.width {
                assert_eq!(
                    extended.image.get(cx + x, cy + y),
                    base.image.get(bx + x, by + y),
                    "{composed} altered the letter at {x},{y}"
                );
            }
        }
    }
}

#[test]
fn the_horn_and_the_stroke_are_recognised_as_vowel_marks() {
    let compositions = vietnamese_compositions();
    assert_eq!(
        compositions
            .iter()
            .find(|c| c.composed == 'ơ')
            .unwrap()
            .vowel_mark,
        Some(VowelMark::Horn)
    );
    assert_eq!(
        compositions
            .iter()
            .find(|c| c.composed == 'đ')
            .unwrap()
            .vowel_mark,
        Some(VowelMark::Stroke)
    );
}
