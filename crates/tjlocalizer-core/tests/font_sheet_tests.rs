//! Reading a glyph sheet and drawing the letters it is missing.
//!
//! The sheets here are built in the test rather than committed, so what is being asserted is
//! visible in the same file as the assertion.

use tjlocalizer_core::font::sheet::{extend, Grid, Image, Sheet};
use tjlocalizer_core::font::{vietnamese_compositions, Coverage};

const CELL: u32 = 12;
const COLUMNS: u32 = 16;

/// A sheet of printable ASCII where each letter is a small block of ink, padded so there is room
/// above and below - which is what a font drawn for a game looks like.
fn ascii_sheet(padding_top: u32) -> Sheet {
    let characters: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    let rows = (characters.len() as u32).div_ceil(COLUMNS);
    let mut image = Image::new(COLUMNS * CELL, rows * CELL);
    let grid = Grid {
        cell_width: CELL,
        cell_height: CELL,
        columns: COLUMNS,
        rows,
    };

    for (i, c) in characters.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let (ox, oy) = grid.cell_origin(i as u32);
        // Each letter gets its own 6x5 pattern, derived from its codepoint. A real font's letters
        // differ from one another, and a fixture where they do not makes "á and Á are drawn
        // identically" true of the fixture rather than of the composition.
        // Mixed so that neighbouring codepoints differ in the interior rather than in bits that
        // land on the border - "a and e are drawn identically" must be a statement about the
        // composition, not about the fixture.
        let seed = (*c as u32).wrapping_mul(2_654_435_761);
        for y in 0..5u32 {
            for x in 0..6u32 {
                // The outer ring is always ink, so every glyph has the same bounds and the same
                // headroom; only the inside varies.
                let edge = x == 0 || x == 5 || y == 0 || y == 4;
                // Computed only off the edge: y - 1 underflows on the top row.
                let inked = edge || {
                    let bit = (y - 1) * 4 + (x - 1).min(3);
                    (seed >> bit) & 1 == 1
                };
                if inked {
                    image.set(ox + 3 + x, oy + padding_top + y, [220, 200, 120, 255]);
                }
            }
        }
    }
    Sheet::ascii(image, grid)
}

fn coverage_of(sheet: &Sheet) -> Coverage {
    Coverage::new(sheet.order.clone(), "test sheet")
}

#[test]
fn a_sheet_reports_what_it_covers() {
    let sheet = ascii_sheet(4);
    assert!(sheet.covers('a'));
    assert!(sheet.covers('~'));
    assert!(!sheet.covers('ế'));
    assert_eq!(sheet.index_of(' '), Some(0));
    assert_eq!(sheet.index_of('A'), Some('A' as u32 - 0x20));
}

/// Marks are placed against the ink, not the cell: cells are padded and the padding differs per
/// glyph, so a mark measured from the cell edge floats away from short letters.
#[test]
fn ink_bounds_find_the_letter_inside_its_padded_cell() {
    let sheet = ascii_sheet(4);
    let bounds = sheet.ink_bounds('a').expect("a has ink");
    assert_eq!(bounds.x, 3);
    assert_eq!(bounds.y, 4);
    assert_eq!(bounds.width, 6);
    assert_eq!(bounds.height, 5);
    assert_eq!(bounds.top(), 4);
    assert_eq!(bounds.bottom(), 9);
}

#[test]
fn a_cell_with_no_ink_has_no_bounds() {
    let sheet = ascii_sheet(4);
    assert!(sheet.ink_bounds(' ').is_none());
}

/// A mark drawn in the wrong colour looks like a defect rather than an omission, so the colour is
/// sampled from the game's own glyph rather than assumed to be black.
#[test]
fn the_ink_colour_is_taken_from_the_game_s_own_glyph() {
    let sheet = ascii_sheet(4);
    assert_eq!(sheet.ink_colour('a'), Some([220, 200, 120, 255]));
}

/// The whole point: a sheet that covers ASCII can have all 134 Vietnamese letters built from it.
#[test]
fn every_vietnamese_letter_is_composed_from_an_ascii_sheet() {
    let sheet = ascii_sheet(4);
    let (extended, report) = extend(&sheet, &vietnamese_compositions()).unwrap();

    assert_eq!(report.added.len(), 134, "skipped: {:?}", report.skipped);
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert!(coverage_of(&extended).missing_for_vietnamese().is_empty());
    assert!(extended.covers('ế'));
    assert!(extended.covers('Ự'));
    assert!(extended.covers('đ'));
}

/// The original glyphs must survive untouched and keep their indices, or a game that indexes into
/// the sheet by position draws the wrong character for text that was already working.
#[test]
fn the_original_glyphs_keep_their_place_and_their_pixels() {
    let sheet = ascii_sheet(4);
    let (extended, _) = extend(&sheet, &vietnamese_compositions()).unwrap();

    for (i, c) in sheet.order.iter().enumerate() {
        assert_eq!(extended.index_of(*c), Some(i as u32), "{c} moved");
    }

    // Compared cell by cell rather than over the whole rectangle: the original sheet's last row
    // has spare cells beyond its last character, and filling those with new glyphs is right.
    for (i, c) in sheet.order.iter().enumerate() {
        let (ox, oy) = sheet.grid.cell_origin(i as u32);
        for y in 0..sheet.grid.cell_height {
            for x in 0..sheet.grid.cell_width {
                assert_eq!(
                    extended.image.get(ox + x, oy + y),
                    sheet.image.get(ox + x, oy + y),
                    "the glyph for {c:?} changed at {x},{y}"
                );
            }
        }
    }
}

#[test]
fn the_sheet_grows_downwards_and_keeps_its_width() {
    let sheet = ascii_sheet(4);
    let (extended, report) = extend(&sheet, &vietnamese_compositions()).unwrap();

    assert_eq!(extended.grid.columns, sheet.grid.columns);
    assert_eq!(extended.image.width, sheet.image.width);
    assert!(extended.grid.rows > sheet.grid.rows);
    assert_eq!(report.rows, extended.grid.rows);
    // Enough cells for everything, and not a row more than needed.
    assert!(extended.grid.capacity() >= extended.order.len() as u32);
    assert!(extended.grid.capacity() - (extended.order.len() as u32) < COLUMNS);
}

/// A composed glyph must carry the base letter's own pixels, or it is not the game's letter any
/// more - which is the entire reason for composing rather than importing a typeface.
#[test]
fn a_composed_glyph_contains_its_base_letter() {
    let sheet = ascii_sheet(4);
    let (extended, _) = extend(&sheet, &vietnamese_compositions()).unwrap();

    let base = sheet.ink_bounds('e').unwrap();
    let (bx, by) = sheet.grid.cell_origin(base.cell);
    let composed = extended.index_of('ế').unwrap();
    let (cx, cy) = extended.grid.cell_origin(composed);

    for y in base.y..base.y + base.height {
        for x in base.x..base.x + base.width {
            assert_eq!(
                extended.image.get(cx + x, cy + y),
                sheet.image.get(bx + x, by + y),
                "the base letter was altered at {x},{y}"
            );
        }
    }
}

/// A tone stacks above a circumflex rather than on top of it, or ế and é become the same picture -
/// and they are different words.
#[test]
fn a_stacked_glyph_has_more_ink_above_the_letter_than_a_plain_one() {
    let sheet = ascii_sheet(5);
    let (extended, report) = extend(&sheet, &vietnamese_compositions()).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);

    let top_of = |c: char| extended.ink_bounds(c).unwrap().top();
    let plain = top_of('e');
    let acute = top_of('é');
    let circumflex = top_of('ê');
    let stacked = top_of('ế');

    assert!(acute < plain, "the acute must sit above the letter");
    assert!(
        circumflex < plain,
        "the circumflex must sit above the letter"
    );
    assert!(
        stacked < circumflex,
        "ế must reach higher than ê, or it is indistinguishable from it"
    );
}

/// Marked and toned letters must not all look the same. Two tones that draw identically are two
/// words that read identically.
#[test]
fn each_tone_draws_differently() {
    let sheet = ascii_sheet(5);
    let (extended, _) = extend(&sheet, &vietnamese_compositions()).unwrap();

    let cell_pixels = |c: char| {
        let index = extended.index_of(c).unwrap();
        let (ox, oy) = extended.grid.cell_origin(index);
        let mut out = Vec::new();
        for y in 0..extended.grid.cell_height {
            for x in 0..extended.grid.cell_width {
                out.push(extended.image.get(ox + x, oy + y));
            }
        }
        out
    };

    let shapes: Vec<Vec<[u8; 4]>> = "aáàảãạ".chars().map(cell_pixels).collect();
    for i in 0..shapes.len() {
        for j in i + 1..shapes.len() {
            assert_ne!(
                shapes[i], shapes[j],
                "two of a á à ả ã ạ are drawn identically"
            );
        }
    }
}

/// A clipped tone mark is a different word, so a glyph with no room is refused and reported
/// rather than drawn badly.
#[test]
fn a_letter_with_no_headroom_is_reported_rather_than_clipped() {
    // Ink starting at the very top of the cell: nowhere to put a mark.
    let sheet = ascii_sheet(0);
    let (_, report) = extend(&sheet, &vietnamese_compositions()).unwrap();

    assert!(!report.skipped.is_empty());
    assert!(
        report.skipped.iter().any(|s| s.composed == 'á'),
        "a mark above cannot be drawn here"
    );
    assert!(
        report.skipped[0].reason.contains("clear rows above"),
        "{:?}",
        report.skipped[0]
    );
    // The stroke through đ needs no headroom, so it is still drawn.
    assert!(report.added.contains(&'đ'), "{:?}", report.added);
}

/// Running it twice must not duplicate anything.
#[test]
fn extending_an_already_extended_sheet_adds_nothing() {
    let sheet = ascii_sheet(4);
    let (once, _) = extend(&sheet, &vietnamese_compositions()).unwrap();
    let (twice, report) = extend(&once, &vietnamese_compositions()).unwrap();

    assert!(report.added.is_empty());
    assert_eq!(twice.order.len(), once.order.len());
}

/// The sheet has to survive a trip through PNG, which is how it reaches and leaves the archive.
#[test]
fn a_sheet_round_trips_through_png() {
    let sheet = ascii_sheet(4);
    let (extended, _) = extend(&sheet, &vietnamese_compositions()).unwrap();

    let encoded = extended.image.encode_png().unwrap();
    let decoded = Image::decode_png(&encoded).unwrap();

    assert_eq!(decoded.width, extended.image.width);
    assert_eq!(decoded.height, extended.image.height);
    assert_eq!(decoded.pixels, extended.image.pixels);
}

/// The harder version of the previous test, and the one that matters more.
///
/// ắ and ấ are different letters - "bắt" and "bất" are different words - and they differ only by
/// the mark under the tone. Two tones on a plain vowel are easy to keep apart; a tone stacked on
/// a breve against the same tone stacked on a circumflex is where a small bitmap font gives up.
#[test]
fn a_breve_and_a_circumflex_stay_distinguishable_under_every_tone() {
    let sheet = ascii_sheet(4);
    let (extended, report) = extend(&sheet, &vietnamese_compositions()).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);

    let cell_pixels = |c: char| {
        let index = extended
            .index_of(c)
            .unwrap_or_else(|| panic!("{c} was not composed"));
        let (ox, oy) = extended.grid.cell_origin(index);
        let mut out = Vec::new();
        for y in 0..extended.grid.cell_height {
            for x in 0..extended.grid.cell_width {
                out.push(extended.image.get(ox + x, oy + y));
            }
        }
        out
    };

    for (breve, circumflex) in [
        ('ă', 'â'),
        ('ắ', 'ấ'),
        ('ằ', 'ầ'),
        ('ẳ', 'ẩ'),
        ('ẵ', 'ẫ'),
        ('ặ', 'ậ'),
        ('Ă', 'Â'),
        ('Ắ', 'Ấ'),
    ] {
        assert_ne!(
            cell_pixels(breve),
            cell_pixels(circumflex),
            "{breve} and {circumflex} are drawn identically - they are different letters"
        );
    }
}

/// Every composed letter must be a distinct picture. Two that draw the same are two words a
/// player cannot tell apart.
#[test]
fn no_two_composed_letters_are_drawn_identically() {
    let sheet = ascii_sheet(4);
    let (extended, report) = extend(&sheet, &vietnamese_compositions()).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);

    let mut seen: std::collections::HashMap<Vec<[u8; 4]>, char> = Default::default();
    for c in &report.added {
        let index = extended.index_of(*c).unwrap();
        let (ox, oy) = extended.grid.cell_origin(index);
        let mut pixels = Vec::new();
        for y in 0..extended.grid.cell_height {
            for x in 0..extended.grid.cell_width {
                pixels.push(extended.image.get(ox + x, oy + y));
            }
        }
        if let Some(other) = seen.insert(pixels, *c) {
            panic!("{c} and {other} are drawn identically");
        }
    }
}

/// A sheet's own grid should be the first thing offered for it.
///
/// The point of ranking is that the top suggestion is usually right; if it were not, a person
/// scanning the list would be no better off than typing numbers into an empty box.
#[test]
fn the_real_grid_ranks_first() {
    let sheet = ascii_sheet(3);
    let guesses = tjlocalizer_core::font::sheet::plausible_grids(&sheet.image);

    assert!(!guesses.is_empty(), "no grid was offered for a real sheet");
    assert_eq!(
        guesses[0].grid, sheet.grid,
        "the sheet's own grid was not the first suggestion"
    );
    assert!(
        guesses[0].fit > 0.9,
        "the right grid scored badly: {}",
        guesses[0].fit
    );
}

/// Every suggestion has to divide the image evenly and hold enough cells to be a character set.
/// A grid that does not is not a near miss - it is a guaranteed misreading of every glyph.
#[test]
fn suggestions_divide_the_image_and_could_hold_a_character_set() {
    let sheet = ascii_sheet(3);
    for guess in tjlocalizer_core::font::sheet::plausible_grids(&sheet.image) {
        let grid = guess.grid;
        assert_eq!(grid.cell_width * grid.columns, sheet.image.width);
        assert_eq!(grid.cell_height * grid.rows, sheet.image.height);
        assert!(
            grid.capacity() >= 64,
            "{grid:?} could not hold printable ASCII"
        );
        assert!((0.0..=1.0).contains(&guess.fit));
    }
}

/// Artwork should not be mistaken for a glyph sheet.
///
/// Not by being rejected - `inspect` reports rather than judges - but by scoring low enough on
/// every measure that it sorts below a real sheet.
#[test]
fn artwork_looks_nothing_like_a_glyph_sheet() {
    let sheet = ascii_sheet(3);
    let sheet_png = sheet.image.encode_png().unwrap();

    // Dense, many-coloured, no clear lines anywhere: a background, not a font.
    let mut art = Image::new(192, 132);
    for y in 0..132u32 {
        for x in 0..192u32 {
            let r = ((x * 7 + y * 3) % 256) as u8;
            let g = ((x * 3 + y * 11) % 256) as u8;
            let b = ((x * 13 + y * 5) % 256) as u8;
            art.set(x, y, [r, g, b, 255]);
        }
    }
    let art_png = art.encode_png().unwrap();

    let a = tjlocalizer_core::font::sheet::inspect("font.png", &sheet_png).unwrap();
    let b = tjlocalizer_core::font::sheet::inspect("sky.png", &art_png).unwrap();

    assert_eq!(a.entry, "font.png");
    assert!(
        a.ink_share < b.ink_share,
        "the sheet was not the emptier image"
    );
    assert!(a.colours < b.colours, "the sheet was not the plainer image");
    let art_fit = b.grids.first().map(|g| g.fit).unwrap_or(0.0);
    assert!(
        a.grids[0].fit > art_fit,
        "artwork fitted a grid as well as a real sheet did ({art_fit})"
    );
}
