//! Seeing what a build changed, and running it in whatever the person has (§25).
//!
//! This tool does not ship an emulator and will not pretend to. Nothing here executes a game,
//! nothing here knows what a menu looks like, and a report that said "the build looks fine"
//! would be a lie about work nobody did.
//!
//! What it can do is two things that are true.
//!
//! The first: it draws every approved translation in the game's own glyphs, at the game's own
//! size (`font::proof`), and that drawing can be kept and compared against the next one. A
//! translator changes six lines and the picture changes in six places; if it changes in sixty,
//! something else moved - a font was recomposed, a glyph order was edited, a rule installed a
//! sheet with a different baseline. That is exactly the class of failure a text report cannot
//! show and a person will not find by reading a diff of `translations.json`.
//!
//! The second: the person testing the build almost certainly has an emulator already, and what
//! they lack is not a JVM but the tedium of finding the newest output and typing the command. So
//! the project records the command *they* chose, and running it is one word.
//!
//! Note what that second part is not. The command is written down by the person, in their own
//! project file, and is never inferred from the game, from an archive, or from anything a
//! download could influence: §29's rule is that nothing extracted is executed, and a launcher
//! that read its command out of a JAR's manifest would break that rule while looking helpful.

use crate::font::sheet::Image;
use serde::{Deserialize, Serialize};

/// What changed between two drawings of the same game's text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Difference {
    /// Set when the two pictures are not the same size, which is a change in itself: a line was
    /// added, or the sheet's letters got taller.
    pub resized: bool,
    pub before: (u32, u32),
    pub after: (u32, u32),
    /// Pixels that differ, over the area compared.
    pub changed: usize,
    pub compared: usize,
    /// Rows of change, which for a proof sheet are the lines that moved.
    pub bands: Vec<Band>,
}

/// A run of rows that changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Band {
    pub top: u32,
    pub bottom: u32,
    pub changed: usize,
}

impl Difference {
    pub fn is_identical(&self) -> bool {
        !self.resized && self.changed == 0
    }

    /// Share of the compared area that changed, 0 to 1.
    pub fn share(&self) -> f32 {
        if self.compared == 0 {
            return 0.0;
        }
        self.changed as f32 / self.compared as f32
    }
}

/// Compares two drawings pixel for pixel.
///
/// Exact comparison, no tolerance. These are two renderings by the same code from the same glyph
/// sheet: any difference at all is a real difference, and a threshold here would only hide the
/// single-pixel baseline shift that is the whole reason to look.
pub fn compare(before: &Image, after: &Image) -> Difference {
    let width = before.width.min(after.width);
    let height = before.height.min(after.height);

    let mut changed = 0usize;
    let mut rows = vec![0usize; height as usize];
    for y in 0..height {
        for x in 0..width {
            if before.get(x, y) != after.get(x, y) {
                changed += 1;
                rows[y as usize] += 1;
            }
        }
    }

    let mut bands = Vec::new();
    let mut start: Option<u32> = None;
    let mut count = 0usize;
    for (y, row) in rows.iter().enumerate() {
        match (*row > 0, start) {
            (true, None) => {
                start = Some(y as u32);
                count = *row;
            }
            (true, Some(_)) => count += *row,
            (false, Some(top)) => {
                bands.push(Band {
                    top,
                    bottom: y as u32 - 1,
                    changed: count,
                });
                start = None;
                count = 0;
            }
            (false, None) => {}
        }
    }
    if let Some(top) = start {
        bands.push(Band {
            top,
            bottom: height.saturating_sub(1),
            changed: count,
        });
    }

    Difference {
        resized: before.width != after.width || before.height != after.height,
        before: (before.width, before.height),
        after: (after.width, after.height),
        changed,
        compared: (width * height) as usize,
        bands,
    }
}

/// A picture of the difference: what is now there, with what changed marked.
///
/// Marked rather than replaced, so the result is still readable as text. A diff image that painted
/// changed pixels solid would show where something moved and hide what it now says, and what it
/// now says is the thing being checked.
pub fn marked(before: &Image, after: &Image) -> Image {
    let mut out = after.clone();
    for y in 0..after.height {
        for x in 0..after.width {
            let same = x < before.width && y < before.height && before.get(x, y) == after.get(x, y);
            if same {
                continue;
            }
            let pixel = after.get(x, y);
            if pixel[3] < 24 {
                // Something that used to be drawn here and is not any more: the loss is invisible
                // in the new picture, so it gets a mark of its own.
                out.set(x, y, [90, 30, 30, 255]);
            } else {
                out.set(x, y, [255, 90, 90, pixel[3]]);
            }
        }
    }
    out
}

/// An emulator a person has, and how they run it.
///
/// Their command, written down in their project. Nothing here suggests one, downloads one, or
/// reads one out of a game.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Emulator {
    /// The program to run.
    pub command: String,
    /// Its arguments. `{game}` is replaced with the path of the build being tested; without it,
    /// the path is appended, because that is what most emulators expect.
    #[serde(default)]
    pub args: Vec<String>,
}

impl Emulator {
    /// The argument list for one build, with `{game}` filled in.
    pub fn arguments(&self, game: &std::path::Path) -> Vec<String> {
        let game = game.display().to_string();
        if self.args.iter().any(|a| a.contains("{game}")) {
            return self
                .args
                .iter()
                .map(|a| a.replace("{game}", &game))
                .collect();
        }
        let mut args = self.args.clone();
        args.push(game);
        args
    }
}
