//! A sheet of the translations as the game will draw them (§25).
//!
//! No emulator here, and this does not pretend to be one: it cannot show a menu, a background or
//! a button. What it can show is the text itself, drawn with the game's own glyphs at the game's
//! own size - which is where the failures this project cares about actually live. A diacritic
//! landing on the letter below it, a stack that reads as a smudge at twelve pixels, a label that
//! runs past where the original stopped: all of those are visible here and in no report.
//!
//! The one thing added that a game would not draw is a marker at the original's width. Nothing
//! knows how wide the button is, but the original fitted it, so where the original ended is the
//! most useful line anybody can draw on this picture.

use super::metrics::Metrics;
use super::sheet::{render_line_with, scaled, Image, Sheet};

/// One line to check: what the game said, and what it will say.
pub struct Row<'a> {
    pub source: &'a str,
    pub target: &'a str,
}

const GAP: u32 = 3;
const MARGIN: u32 = 6;
const BACKGROUND: [u8; 4] = [16, 18, 24, 255];
const RULE: [u8; 4] = [48, 54, 66, 255];
const MARKER: [u8; 4] = [190, 120, 60, 255];

/// Draws every row, original above translation, enlarged so a person can see it.
pub fn sheet(sheet_: &Sheet, metrics: &Metrics, rows: &[Row], scale: u32) -> Image {
    let scale = scale.clamp(1, 8);
    let mut drawn: Vec<(Image, Image, u32)> = Vec::new();

    for row in rows {
        let source = scaled(&render_line_with(sheet_, metrics, row.source), scale);
        let target = scaled(&render_line_with(sheet_, metrics, row.target), scale);
        let marker = source.width;
        drawn.push((source, target, marker));
    }

    let width = drawn
        .iter()
        .map(|(a, b, _)| a.width.max(b.width))
        .max()
        .unwrap_or(1)
        + MARGIN * 2;
    let height: u32 = drawn
        .iter()
        .map(|(a, b, _)| a.height + b.height + GAP * 3)
        .sum::<u32>()
        + MARGIN * 2;

    let mut out = Image::new(width, height.max(1));
    for y in 0..out.height {
        for x in 0..out.width {
            out.set(x, y, BACKGROUND);
        }
    }

    let mut y = MARGIN;
    for (source, target, marker) in &drawn {
        blit(&mut out, source, MARGIN, y);
        y += source.height + GAP;

        // The line where the original stopped, drawn behind the translation rather than beside
        // it: an overflow is then something you see rather than something you measure.
        let x = MARGIN + marker;
        if x < out.width {
            for ty in y..(y + target.height).min(out.height) {
                out.set(x, ty, MARKER);
            }
        }
        blit(&mut out, target, MARGIN, y);
        y += target.height + GAP;

        for rx in 0..out.width {
            if y < out.height {
                out.set(rx, y, RULE);
            }
        }
        y += GAP;
    }
    out
}

/// Copies an image in, leaving the background where the source is transparent.
fn blit(out: &mut Image, image: &Image, x: u32, y: u32) {
    for iy in 0..image.height {
        for ix in 0..image.width {
            let pixel = image.get(ix, iy);
            if pixel[3] >= 24 && x + ix < out.width && y + iy < out.height {
                out.set(x + ix, y + iy, pixel);
            }
        }
    }
}
