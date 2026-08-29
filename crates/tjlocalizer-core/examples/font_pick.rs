//! Scans a folder of fonts and ranks them against a sheet of a given cell size.
use tjlocalizer_core::font::library;
use tjlocalizer_core::font::sheet::{Grid, Image, Sheet};

fn sheet(cell: u32, ink: u32, pad: u32) -> Sheet {
    let chars: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    let cols = 16u32;
    let rows = (chars.len() as u32).div_ceil(cols);
    let mut image = Image::new(cols * cell, rows * cell);
    let grid = Grid {
        cell_width: cell,
        cell_height: cell,
        columns: cols,
        rows,
    };
    for (i, c) in chars.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let (ox, oy) = grid.cell_origin(i as u32);
        let seed = (*c as u32).wrapping_mul(2_654_435_761);
        let w = ink.min(6);
        for y in 0..ink {
            for x in 0..w {
                let edge = x == 0 || x + 1 == w || y == 0 || y + 1 == ink;
                if edge || (seed >> ((y * 4 + x) % 24)) & 1 == 1 {
                    image.set(ox + 2 + x, oy + pad + y, [255, 255, 255, 255]);
                }
            }
        }
    }
    Sheet::ascii(image, grid)
}

fn main() {
    let dir = std::env::args().nth(1).expect("font directory");
    let cell: u32 = std::env::args()
        .nth(2)
        .map(|s| s.parse().unwrap())
        .unwrap_or(12);
    let (ink, pad) = (cell * 5 / 12, cell * 5 / 12);

    let found = library::scan(std::path::Path::new(&dir)).expect("scan");
    let full = found.iter().filter(|c| c.covers_vietnamese).count();
    println!(
        "{} fonts readable, {full} cover all 134 Vietnamese letters",
        found.len()
    );

    let s = sheet(cell, ink.max(4), pad.max(4));
    let candidates: Vec<_> = found.into_iter().filter(|c| c.covers_vietnamese).collect();
    let fits = library::rank(&s, &candidates).expect("rank");
    println!("--- best for a {cell}px cell:");
    for fit in fits.iter().take(8) {
        println!(
            "  {:>3}/{:<3} ({:.0}%)  {}",
            fit.from_typeface,
            fit.composed,
            fit.share() * 100.0,
            fit.name
        );
    }
    println!("--- worst:");
    for fit in fits.iter().rev().take(3) {
        println!(
            "  {:>3}/{:<3} ({:.0}%)  {}",
            fit.from_typeface,
            fit.composed,
            fit.share() * 100.0,
            fit.name
        );
    }
}
