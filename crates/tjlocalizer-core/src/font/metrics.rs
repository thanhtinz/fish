//! How wide a line of text will be, in the game's own pixels (§24).
//!
//! Character counts are the wrong unit for this question, and the more proportional a font is the
//! wronger they get. "Menu" and "Illli" are both five characters; in most sheets one is half the
//! width of the other. A translation flagged as too long because it gained two letters, or missed
//! because it gained only one but that one was a W, is a check that costs trust without catching
//! overflow.
//!
//! So the sheet is measured. Every cell's ink is bounded, which gives a width per character, and a
//! line is the sum of those widths - which is exactly what the game does when it draws the line,
//! give or take the one pixel it puts between letters.
//!
//! The honest limit: nothing here knows how wide the button is. What it knows is that the original
//! text fitted, because the game shipped that way, so a translation much wider than the original
//! is a risk. That is a weaker claim than "this overflows" and it is the claim the data supports.

use super::sheet::Sheet;
use std::collections::BTreeMap;

/// What a game's sheet says about the width of its own letters.
#[derive(Debug, Clone)]
pub struct Metrics {
    /// Ink width per character. A character with no ink - the space - is not in here.
    widths: BTreeMap<char, u32>,
    /// What a space costs. Taken from the cell, since a space has no ink to measure.
    space: u32,
    /// Pixels between letters, which is what a sheet-drawing game almost always adds.
    tracking: u32,
    /// True when every letter is the width of its cell, so a character count would have done.
    pub monospaced: bool,
    pub cell_width: u32,
}

impl Metrics {
    /// Reads the widths out of a sheet.
    pub fn of(sheet: &Sheet) -> Self {
        let mut widths = BTreeMap::new();
        for c in &sheet.order {
            if let Some(bounds) = sheet.ink_bounds(*c) {
                widths.insert(*c, bounds.width);
            }
        }

        // A sheet whose letters all measure the same is a sheet the game draws on a fixed pitch,
        // and saying so lets the caller fall back to counting characters rather than pretending
        // to a precision the sheet does not have.
        let mut seen: Vec<u32> = widths.values().copied().collect();
        seen.sort_unstable();
        seen.dedup();
        let monospaced = seen.len() <= 1;

        Metrics {
            space: sheet.grid.cell_width / 2,
            tracking: 1,
            monospaced,
            cell_width: sheet.grid.cell_width,
            widths,
        }
    }

    /// The width of one character, or `None` when the sheet has no glyph for it.
    pub fn width_of(&self, c: char) -> Option<u32> {
        if c == ' ' {
            return Some(self.space);
        }
        self.widths.get(&c).copied()
    }

    /// The width of a line, or `None` when any character is one the sheet cannot draw.
    ///
    /// `None` rather than a guess: a string with a missing glyph has a bigger problem than its
    /// width, the font check reports that one, and inventing a width for it here would produce a
    /// second, misleading complaint about the same string.
    pub fn measure(&self, text: &str) -> Option<u32> {
        let mut total = 0u32;
        let mut count = 0u32;
        for c in text.chars() {
            if c == '\n' {
                // Measured per line: a wrapped string is as wide as its widest line, not as wide
                // as all of them laid end to end.
                return text
                    .lines()
                    .map(|line| self.measure(line))
                    .collect::<Option<Vec<u32>>>()
                    .map(|widths| widths.into_iter().max().unwrap_or(0));
            }
            total += self.width_of(c)?;
            count += 1;
        }
        Some(total + self.tracking * count.saturating_sub(1))
    }
}
