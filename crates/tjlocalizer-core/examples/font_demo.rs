//! Renders a sheet of composed Vietnamese glyphs so the marks can be looked at.
//!
//! The tests assert that the tones differ from one another; only an eye can say whether they are
//! the right shapes at this size.

use tjlocalizer_core::font::sheet::{extend, Grid, Image, Sheet};
use tjlocalizer_core::font::vietnamese_compositions;

/// A 5x7 blocky letter for each ASCII character, in the style of a J2ME game's own font.
fn stroke_font() -> Sheet {
    const COLUMNS: u32 = 16;
    const CELL_W: u32 = 10;
    const CELL_H: u32 = 16;

    let characters: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    let rows = (characters.len() as u32).div_ceil(COLUMNS);
    let mut image = Image::new(COLUMNS * CELL_W, rows * CELL_H);
    let grid = Grid {
        cell_width: CELL_W,
        cell_height: CELL_H,
        columns: COLUMNS,
        rows,
    };

    // Five-by-seven shapes for the letters this demo shows; everything else gets a filled box so
    // the sheet is complete.
    for (i, c) in characters.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let (ox, oy) = grid.cell_origin(i as u32);
        let rows_of: [u8; 7] = match c {
            'a' => [
                0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b10011, 0b01101,
            ],
            'e' => [
                0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b10001, 0b01110,
            ],
            'o' => [
                0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            'u' => [
                0b00000, 0b10001, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101,
            ],
            'y' => [
                0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
            ],
            'i' => [
                0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110,
            ],
            'd' => [
                0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b10011, 0b01101,
            ],
            'A' => [
                0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
            ],
            'E' => [
                0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
            ],
            'O' => [
                0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            'U' => [
                0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
            ],
            'I' => [
                0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
            ],
            'Y' => [
                0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
            ],
            'D' => [
                0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
            ],
            'B' => [
                0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
            ],
            'b' => [
                0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b11110,
            ],
            'c' => [
                0b00000, 0b01110, 0b10001, 0b10000, 0b10000, 0b10001, 0b01110,
            ],
            'h' => [
                0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001,
            ],
            'n' => [
                0b00000, 0b00000, 0b11110, 0b10001, 0b10001, 0b10001, 0b10001,
            ],
            'r' => [
                0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000,
            ],
            't' => [
                0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00101, 0b00010,
            ],
            _ => [0b01110; 7],
        };
        for (y, bits) in rows_of.iter().enumerate() {
            for x in 0..5u32 {
                if bits & (1 << (4 - x)) != 0 {
                    image.set(ox + 2 + x, oy + 6 + y as u32, [235, 232, 225, 255]);
                }
            }
        }
    }
    Sheet::ascii(image, grid)
}

/// Draws a line of text with a sheet, the way a game would: look the character up, blit its cell.
///
/// This is the check the unit tests cannot make. They assert that the tones differ from one
/// another; only reading a word says whether the marks are the right shapes at this size.
fn render_line(sheet: &Sheet, text: &str) -> Image {
    let advance = sheet.grid.cell_width;
    let mut image = Image::new(
        advance * text.chars().count() as u32,
        sheet.grid.cell_height,
    );
    for (i, c) in text.chars().enumerate() {
        let Some(index) = sheet.index_of(c) else {
            continue;
        };
        let (ox, oy) = sheet.grid.cell_origin(index);
        for y in 0..sheet.grid.cell_height {
            for x in 0..advance {
                image.set(i as u32 * advance + x, y, sheet.image.get(ox + x, oy + y));
            }
        }
    }
    image
}

fn scaled(image: &Image, scale: u32) -> Image {
    let mut big = Image::new(image.width * scale, image.height * scale);
    for y in 0..big.height {
        for x in 0..big.width {
            big.set(x, y, image.get(x / scale, y / scale));
        }
    }
    big
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "font.png".into());
    let sheet = stroke_font();
    let (extended, report) = extend(&sheet, &vietnamese_compositions()).expect("composition");

    println!(
        "{} glyphs added, {} skipped",
        report.added.len(),
        report.skipped.len()
    );
    for skipped in &report.skipped {
        println!("  {} - {}", skipped.composed, skipped.reason);
    }

    // Written scaled up: these glyphs are 5 pixels wide and the marks are two, which is legible
    // on a phone and invisible on a monitor.
    std::fs::write(
        &out,
        scaled(&extended.image, 6).encode_png().expect("encode"),
    )
    .expect("write");
    println!("wrote {out}");

    for (name, text) in [
        ("line-1", "Bắt đầu trò chơi"),
        ("line-2", "Sinh lực Trang bị"),
        ("line-3", "a ă â  á à ả ã ạ  ắ ằ ẳ ẵ ặ  ấ ầ ẩ ẫ ậ"),
        ("line-4", "e ê  é è ẻ ẽ ẹ  ế ề ể ễ ệ  đ Đ ơ ư"),
    ] {
        let line = render_line(&extended, text);
        let path = out.replace(".png", &format!("-{name}.png"));
        std::fs::write(&path, scaled(&line, 8).encode_png().expect("encode")).expect("write");
        println!("wrote {path}");
    }
}
