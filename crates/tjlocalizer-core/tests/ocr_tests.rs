//! Reading the words painted into artwork, against the game's own glyph sheet.
//!
//! The images here are drawn with the same sheet the reader is given, which is exactly the
//! situation the reader is for: a button label in a game is drawn with the game's font.

use tjlocalizer_core::assets::ocr;
use tjlocalizer_core::font::sheet::{render_line, Grid, Image, Sheet};

const CELL: u32 = 10;
const COLUMNS: u32 = 16;

/// A sheet of printable ASCII where every letter is a different arrangement of pixels, the way a
/// hand-drawn game font is. Letters differ in their outline as well as their interior, because a
/// fixture where every glyph is the same block with a different middle would make this test a
/// statement about noise rather than about letters.
fn ascii_sheet() -> Sheet {
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
        let seed = (*c as u32).wrapping_mul(2_246_822_519) ^ (*c as u32).wrapping_mul(0x9E37_79B9);
        // Widths and heights vary per letter, as they do in any font that is not a grid of boxes.
        let width = 4 + (seed >> 3) % 3;
        let height = 5 + (seed >> 7) % 2;
        for y in 0..height {
            for x in 0..width {
                let bit = (y * width + x) % 31;
                if (seed >> bit) & 1 == 1 || x == 0 || y == height - 1 {
                    image.set(ox + 2 + x, oy + 2 + y, [30, 30, 30, 255]);
                }
            }
        }
    }
    Sheet::ascii(image, grid)
}

/// A label image: text drawn with the sheet, on a background, the way artwork ships.
fn label(sheet: &Sheet, text: &str, opaque: bool) -> Image {
    let drawn = render_line(sheet, text);
    let mut image = Image::new(drawn.width + 6, drawn.height + 4);
    if opaque {
        for y in 0..image.height {
            for x in 0..image.width {
                image.set(x, y, [200, 40, 40, 255]);
            }
        }
    }
    for y in 0..drawn.height {
        for x in 0..drawn.width {
            let pixel = drawn.get(x, y);
            if pixel[3] >= 24 {
                image.set(x + 3, y + 2, pixel);
            }
        }
    }
    image
}

#[test]
fn a_label_drawn_with_the_game_font_reads_back() {
    let sheet = ascii_sheet();
    let image = label(&sheet, "START", false);

    let reading = ocr::read("btn_start.png", &image, &sheet);
    assert_eq!(reading.text(), "START");
    assert!(reading.is_complete(), "{reading:?}");
    assert!(reading.confidence > 0.9, "{}", reading.confidence);
}

#[test]
fn a_label_on_an_opaque_background_reads_back() {
    let sheet = ascii_sheet();
    let image = label(&sheet, "EXIT", true);

    let reading = ocr::read("menu.png", &image, &sheet);
    assert_eq!(reading.text(), "EXIT");
    assert!(reading.is_complete());
}

#[test]
fn spaces_between_words_are_read_as_spaces() {
    let sheet = ascii_sheet();
    let image = label(&sheet, "NEW GAME", false);

    let reading = ocr::read("btn.png", &image, &sheet);
    assert_eq!(reading.text(), "NEW GAME");
}

#[test]
fn two_lines_are_read_as_two_lines() {
    let sheet = ascii_sheet();
    let top = label(&sheet, "GAME", false);
    let bottom = label(&sheet, "OVER", false);

    let mut image = Image::new(top.width.max(bottom.width), top.height + bottom.height + 3);
    for y in 0..top.height {
        for x in 0..top.width {
            image.set(x, y, top.get(x, y));
        }
    }
    for y in 0..bottom.height {
        for x in 0..bottom.width {
            image.set(x, y + top.height + 3, bottom.get(x, y));
        }
    }

    let reading = ocr::read("gameover.png", &image, &sheet);
    assert_eq!(reading.lines.len(), 2);
    assert_eq!(reading.text(), "GAME\nOVER");
}

/// Artwork lettered by hand is not in the sheet, and the answer to that is silence.
///
/// The point of the whole module is that it refuses rather than guesses; a reader that returns
/// its best effort for a shape it has never seen is worse than no reader, because somebody will
/// believe it.
#[test]
fn a_shape_that_is_not_in_the_sheet_is_not_guessed() {
    let sheet = ascii_sheet();
    let mut image = Image::new(20, 12);
    // A blob no letter of this sheet draws: solid, and taller and wider than any of them.
    for y in 2..10u32 {
        for x in 2..12u32 {
            image.set(x, y, [0, 0, 0, 255]);
        }
    }

    let reading = ocr::read("logo.png", &image, &sheet);
    assert!(!reading.is_complete(), "{reading:?}");
    assert!(reading.unread > 0);
    assert!(reading.text().contains('\u{fffd}'));
}

/// An image with nothing in it is not a reading of an empty string.
#[test]
fn an_empty_image_reads_as_nothing() {
    let sheet = ascii_sheet();
    let image = Image::new(16, 16);

    let reading = ocr::read("blank.png", &image, &sheet);
    assert!(reading.lines.is_empty());
    assert!(!reading.is_complete());
    assert_eq!(reading.confidence, 0.0);
}

/// Every glyph the sheet has, drawn alone, reads back as itself.
///
/// This is the property the module lives or dies by, and a per-letter check is the only way to
/// see it: a reader can score well on one word by luck.
#[test]
fn every_letter_reads_back_as_itself() {
    let sheet = ascii_sheet();
    let mut wrong = Vec::new();
    for c in sheet.order.clone() {
        if c == ' ' {
            continue;
        }
        let image = label(&sheet, &c.to_string(), false);
        let reading = ocr::read("one.png", &image, &sheet);
        if reading.text() != c.to_string() {
            wrong.push((c, reading.text()));
        }
    }
    assert!(wrong.is_empty(), "misread: {wrong:?}");
}
