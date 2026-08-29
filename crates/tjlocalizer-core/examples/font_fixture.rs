//! Writes a small ASCII glyph sheet, the shape a J2ME game ships.
use tjlocalizer_core::font::sheet::{Grid, Image, Sheet};

fn main() {
    let out = std::env::args().nth(1).expect("output path");
    let characters: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    let (cw, ch, cols) = (8u32, 12u32, 16u32);
    let rows = (characters.len() as u32).div_ceil(cols);
    let mut image = Image::new(cols * cw, rows * ch);
    let grid = Grid {
        cell_width: cw,
        cell_height: ch,
        columns: cols,
        rows,
    };

    for (i, c) in characters.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let (ox, oy) = grid.cell_origin(i as u32);
        let seed = (*c as u32).wrapping_mul(2_654_435_761);

        // Proportional, like the fonts games actually ship: an i is not as wide as a W, and a
        // fixture where they were would make the layout check (§24) untestable against it.
        let width = match c {
            'i' | 'l' | 'I' | 'j' | '.' | ',' | ':' | ';' | '\'' | '!' | '|' => 2,
            'W' | 'M' | 'm' | 'w' | '@' => 6,
            c if c.is_ascii_uppercase() => 5,
            _ => 4,
        };

        for y in 0..5u32 {
            for x in 0..width {
                let edge = x == 0 || x == width - 1 || y == 0 || y == 4;
                let inked = edge || {
                    let b = (y - 1) * 3 + (x - 1).min(2);
                    (seed >> b) & 1 == 1
                };
                if inked {
                    image.set(ox + 1 + x, oy + 4 + y, [255, 255, 255, 255]);
                }
            }
        }
    }
    let sheet = Sheet::ascii(image, grid);
    std::fs::write(&out, sheet.image.encode_png().unwrap()).unwrap();
    println!("{out}: {}x{}", sheet.image.width, sheet.image.height);
}
