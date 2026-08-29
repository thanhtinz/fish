//! Bitmap glyph sheets: reading one, and drawing the letters it is missing.
//!
//! A J2ME game that does not use the device font draws its text from a PNG of glyphs laid out on
//! a grid. To show Vietnamese it needs 134 more of them, and the only way for those to look like
//! they belong is to build each one from the game's own letters: take its `e`, put its own kind
//! of mark above, and the result has the game's weight, its pixel grid and its personality. A
//! glyph borrowed from a real typeface next to hand-drawn game text looks exactly like what it is.
//!
//! Nothing here decides *where* a game's font lives or how it indexes into it. Those are
//! game-specific and belong to rules and plugins; this module is handed a sheet and a geometry.

use crate::font::{Composition, Tone, VowelMark};
use crate::Result;
use serde::{Deserialize, Serialize};

/// A glyph sheet in memory, as straight RGBA.
#[derive(Debug, Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Four bytes per pixel, row-major.
    pub pixels: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    pub fn decode_png(bytes: &[u8]) -> Result<Self> {
        let decoder = png::Decoder::new(bytes);
        let mut reader = decoder.read_info().map_err(png_error)?;
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buffer).map_err(png_error)?;

        let width = info.width;
        let height = info.height;
        let mut pixels = vec![0u8; (width * height * 4) as usize];

        // Normalised to RGBA here so every later step works on one shape. Game sheets come as
        // palette, greyscale and RGB about as often as RGBA.
        let source = &buffer[..info.buffer_size()];
        let channels = info.color_type.samples();
        for i in 0..(width * height) as usize {
            let (r, g, b, a) = match info.color_type {
                png::ColorType::Grayscale => {
                    let v = source[i];
                    (v, v, v, 255)
                }
                png::ColorType::GrayscaleAlpha => {
                    let v = source[i * 2];
                    (v, v, v, source[i * 2 + 1])
                }
                png::ColorType::Rgb => (source[i * 3], source[i * 3 + 1], source[i * 3 + 2], 255),
                png::ColorType::Rgba => (
                    source[i * 4],
                    source[i * 4 + 1],
                    source[i * 4 + 2],
                    source[i * 4 + 3],
                ),
                png::ColorType::Indexed => {
                    // read_info expands indexed images to RGB or RGBA, so reaching here means the
                    // decoder gave back something unexpected rather than a palette to resolve.
                    let base = i * channels;
                    (
                        source[base],
                        source[base + 1],
                        source[base + 2],
                        if channels > 3 { source[base + 3] } else { 255 },
                    )
                }
            };
            pixels[i * 4] = r;
            pixels[i * 4 + 1] = g;
            pixels[i * 4 + 2] = b;
            pixels[i * 4 + 3] = a;
        }

        Ok(Image {
            width,
            height,
            pixels,
        })
    }

    pub fn encode_png(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().map_err(png_error)?;
            writer.write_image_data(&self.pixels).map_err(png_error)?;
        }
        Ok(out)
    }

    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }

    pub fn set(&mut self, x: u32, y: u32, colour: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let i = ((y * self.width + x) * 4) as usize;
        self.pixels[i..i + 4].copy_from_slice(&colour);
    }
}

fn png_error(e: impl std::fmt::Display) -> crate::Error {
    crate::Error::InvalidProject {
        path: std::path::PathBuf::from("<font sheet>"),
        reason: format!("the glyph sheet could not be read: {e}"),
    }
}

/// How glyphs are laid out on a sheet.
///
/// Given rather than guessed at wherever possible: a grid inferred from one game's sheet is a
/// guess about that sheet, and a wrong guess silently shifts every glyph by a pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grid {
    pub cell_width: u32,
    pub cell_height: u32,
    pub columns: u32,
    pub rows: u32,
}

impl Grid {
    /// The pixel origin of one cell.
    pub fn cell_origin(&self, index: u32) -> (u32, u32) {
        let column = index % self.columns;
        let row = index / self.columns;
        (column * self.cell_width, row * self.cell_height)
    }

    pub fn capacity(&self) -> u32 {
        self.columns * self.rows
    }
}

/// A sheet, its grid, and which character each cell holds.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub image: Image,
    pub grid: Grid,
    /// Cell index for each character, in the order the sheet lays them out.
    pub order: Vec<char>,
    /// The colour treated as nothing. Transparent for most sheets; some use a key colour.
    pub background: [u8; 4],
}

impl Sheet {
    /// A sheet laid out for a run of characters, `columns` wide.
    pub fn new(image: Image, grid: Grid, order: Vec<char>, background: [u8; 4]) -> Self {
        Self {
            image,
            grid,
            order,
            background,
        }
    }

    /// A sheet holding printable ASCII in codepoint order, which is what most game sheets are.
    pub fn ascii(image: Image, grid: Grid) -> Self {
        Self::new(
            image,
            grid,
            (0x20u8..=0x7E).map(|b| b as char).collect(),
            [0, 0, 0, 0],
        )
    }

    pub fn index_of(&self, c: char) -> Option<u32> {
        self.order.iter().position(|g| *g == c).map(|i| i as u32)
    }

    pub fn covers(&self, c: char) -> bool {
        self.index_of(c).is_some()
    }

    /// Whether a pixel counts as ink rather than background.
    fn is_ink(&self, colour: [u8; 4]) -> bool {
        if colour[3] < 24 {
            return false;
        }
        if self.background[3] == 0 {
            return true;
        }
        // A key colour matches loosely: sheets get resaved and lose exactness.
        (0..3).any(|i| colour[i].abs_diff(self.background[i]) > 24)
    }

    /// The tight box around a glyph's ink, within its cell.
    ///
    /// Marks are placed against the ink, not the cell: a cell is padded and the padding differs
    /// per glyph, so a mark positioned from the cell edge floats away from short letters.
    pub fn ink_bounds(&self, c: char) -> Option<InkBounds> {
        let index = self.index_of(c)?;
        let (ox, oy) = self.grid.cell_origin(index);

        let (mut min_x, mut min_y) = (u32::MAX, u32::MAX);
        let (mut max_x, mut max_y) = (0u32, 0u32);
        let mut found = false;

        for y in 0..self.grid.cell_height {
            for x in 0..self.grid.cell_width {
                if self.is_ink(self.image.get(ox + x, oy + y)) {
                    found = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }

        // Computed only when there was ink: `then_some` evaluates its argument eagerly, and with
        // no ink min_x is u32::MAX, so the subtraction overflows. A blank cell is normal - the
        // space character is one - so this is reached on any ordinary sheet.
        if !found {
            return None;
        }
        Some(InkBounds {
            cell: index,
            x: min_x,
            y: min_y,
            width: max_x - min_x + 1,
            height: max_y - min_y + 1,
        })
    }

    /// The colour a glyph is drawn in: the ink pixel that occurs most often.
    ///
    /// Sampled rather than assumed black, because a game's font may be white, gold or outlined,
    /// and a mark in the wrong colour is worse than a missing one - it looks like a defect
    /// rather than an omission.
    pub fn ink_colour(&self, c: char) -> Option<[u8; 4]> {
        let index = self.index_of(c)?;
        let (ox, oy) = self.grid.cell_origin(index);
        let mut counts: std::collections::HashMap<[u8; 4], usize> = Default::default();

        for y in 0..self.grid.cell_height {
            for x in 0..self.grid.cell_width {
                let colour = self.image.get(ox + x, oy + y);
                if self.is_ink(colour) && colour[3] > 200 {
                    *counts.entry(colour).or_default() += 1;
                }
            }
        }
        counts
            .into_iter()
            .max_by_key(|(_, n)| *n)
            .map(|(colour, _)| colour)
    }
}

/// Where a glyph's ink actually sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InkBounds {
    pub cell: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl InkBounds {
    pub fn top(&self) -> u32 {
        self.y
    }
    pub fn bottom(&self) -> u32 {
        self.y + self.height
    }
    pub fn centre_x(&self) -> u32 {
        self.x + self.width / 2
    }
}

/// One glyph that could not be drawn, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub composed: char,
    pub reason: String,
}

/// What extending a sheet produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    pub added: Vec<char>,
    pub skipped: Vec<Skipped>,
    pub columns: u32,
    pub rows: u32,
}

/// Builds a new sheet holding the original glyphs followed by the composed ones.
///
/// The original cells are copied byte for byte and keep their indices, so a game that indexes
/// into the sheet by position still finds every character it had. New glyphs are appended, which
/// means the game must be told about them - that wiring is a per-game patch and belongs to the
/// rule engine, not here. This function makes the glyphs; it does not claim to install them.
pub fn extend(sheet: &Sheet, compositions: &[Composition]) -> Result<(Sheet, Extension)> {
    let grid = sheet.grid;
    let mut order = sheet.order.clone();
    let mut added = Vec::new();
    let mut skipped = Vec::new();
    let mut plan: Vec<(&Composition, u32)> = Vec::new();

    for composition in compositions {
        if sheet.covers(composition.composed) {
            continue;
        }
        let Some(base) = sheet.ink_bounds(composition.base) else {
            skipped.push(Skipped {
                composed: composition.composed,
                reason: format!(
                    "the sheet has no ink for the base letter {:?}",
                    composition.base
                ),
            });
            continue;
        };
        // A mark above needs clear rows over the letter, and one below needs them under. Without
        // room the mark would be clipped or would overwrite the letter, and a clipped tone mark is
        // a different word.
        if let Some(reason) = no_room(&base, composition, grid.cell_height) {
            skipped.push(Skipped {
                composed: composition.composed,
                reason,
            });
            continue;
        }
        plan.push((composition, order.len() as u32));
        order.push(composition.composed);
        added.push(composition.composed);
    }

    let columns = grid.columns;
    let rows = (order.len() as u32).div_ceil(columns).max(grid.rows);
    let mut image = Image::new(columns * grid.cell_width, rows * grid.cell_height);

    // The original sheet is copied wholesale, so nothing about the glyphs a game already draws
    // can change.
    for y in 0..sheet.image.height.min(image.height) {
        for x in 0..sheet.image.width.min(image.width) {
            image.set(x, y, sheet.image.get(x, y));
        }
    }

    let mut extended = Sheet::new(image, Grid { rows, ..grid }, order, sheet.background);

    for (composition, index) in plan {
        draw(sheet, &mut extended, composition, index);
    }

    Ok((
        extended,
        Extension {
            added,
            skipped,
            columns,
            rows,
        },
    ))
}

/// Whether a composed glyph has room for its marks.
fn no_room(base: &InkBounds, composition: &Composition, cell_height: u32) -> Option<String> {
    let above = composition
        .vowel_mark
        .is_some_and(|m| m != VowelMark::Stroke)
        || composition.tone.is_some_and(|t| !t.is_below());
    let stacked = composition
        .vowel_mark
        .is_some_and(|m| matches!(m, VowelMark::Breve | VowelMark::Circumflex))
        && composition.tone.is_some_and(|t| !t.is_below());

    let wanted_above = if stacked {
        4
    } else if above {
        2
    } else {
        0
    };
    if base.top() < wanted_above {
        return Some(format!(
            "only {} clear rows above the letter, {wanted_above} needed",
            base.top()
        ));
    }

    if composition.tone.is_some_and(|t| t.is_below()) && base.bottom() + 2 > cell_height {
        return Some(format!(
            "only {} clear rows below the letter, 2 needed",
            cell_height.saturating_sub(base.bottom())
        ));
    }
    None
}

/// Copies the base letter into its new cell and adds the marks.
fn draw(source: &Sheet, target: &mut Sheet, composition: &Composition, index: u32) {
    let Some(base) = source.ink_bounds(composition.base) else {
        return;
    };
    let colour = source
        .ink_colour(composition.base)
        .unwrap_or([0, 0, 0, 255]);
    let (sx, sy) = source.grid.cell_origin(base.cell);
    let (dx, dy) = target.grid.cell_origin(index);

    for y in 0..source.grid.cell_height {
        for x in 0..source.grid.cell_width {
            target
                .image
                .set(dx + x, dy + y, source.image.get(sx + x, sy + y));
        }
    }

    // The vowel modification goes on first; the tone stacks above whatever is there.
    let mut ceiling = base.top();
    if let Some(mark) = composition.vowel_mark {
        ceiling = draw_vowel_mark(target, dx, dy, &base, mark, colour);
    }
    if let Some(tone) = composition.tone {
        draw_tone(target, dx, dy, &base, ceiling, tone, colour);
    }
}

/// Draws the vowel modification and returns the new topmost ink row.
fn draw_vowel_mark(
    target: &mut Sheet,
    dx: u32,
    dy: u32,
    base: &InkBounds,
    mark: VowelMark,
    colour: [u8; 4],
) -> u32 {
    let centre = base.centre_x();
    let top = base.top();

    match mark {
        // A caret: two pixels rising to a point.
        VowelMark::Circumflex => {
            let y = top.saturating_sub(2);
            target.image.set(dx + centre, dy + y, colour);
            target
                .image
                .set(dx + centre.saturating_sub(1), dy + y + 1, colour);
            target.image.set(dx + centre + 1, dy + y + 1, colour);
            y
        }
        // A cup: the caret upside down.
        VowelMark::Breve => {
            let y = top.saturating_sub(2);
            target
                .image
                .set(dx + centre.saturating_sub(1), dy + y, colour);
            target.image.set(dx + centre + 1, dy + y, colour);
            target.image.set(dx + centre, dy + y + 1, colour);
            y
        }
        // A tick at the upper right of the bowl, which is what a horn is.
        VowelMark::Horn => {
            let x = base.x + base.width;
            target.image.set(dx + x, dy + top, colour);
            target.image.set(dx + x, dy + top.saturating_sub(1), colour);
            top
        }
        // A bar through the ascender, a third of the way down.
        VowelMark::Stroke => {
            let y = top + base.height / 3;
            for x in base.x.saturating_sub(1)..=(base.x + base.width / 2) {
                target.image.set(dx + x, dy + y, colour);
            }
            top
        }
    }
}

fn draw_tone(
    target: &mut Sheet,
    dx: u32,
    dy: u32,
    base: &InkBounds,
    ceiling: u32,
    tone: Tone,
    colour: [u8; 4],
) {
    let centre = base.centre_x();

    match tone {
        Tone::DotBelow => {
            let y = base.bottom() + 1;
            target.image.set(dx + centre, dy + y, colour);
        }
        Tone::Acute => {
            let y = ceiling.saturating_sub(2);
            target.image.set(dx + centre, dy + y + 1, colour);
            target.image.set(dx + centre + 1, dy + y, colour);
        }
        Tone::Grave => {
            let y = ceiling.saturating_sub(2);
            target.image.set(dx + centre, dy + y + 1, colour);
            target
                .image
                .set(dx + centre.saturating_sub(1), dy + y, colour);
        }
        // A small hook: up, then over. Distinguishable from the tilde at this size, which matters
        // because they mark different tones and the words differ.
        Tone::HookAbove => {
            let y = ceiling.saturating_sub(2);
            target.image.set(dx + centre, dy + y + 1, colour);
            target.image.set(dx + centre, dy + y, colour);
            target.image.set(dx + centre + 1, dy + y, colour);
        }
        // A wave, four pixels wide. Drawn as a caret it is pixel for pixel a circumflex, and
        // "ân" and "ãn" are different words - so the extra width is not decoration, it is the
        // only thing keeping the two letters apart.
        Tone::Tilde => {
            let y = ceiling.saturating_sub(2);
            let left = centre.saturating_sub(1);
            target.image.set(dx + centre, dy + y, colour);
            target.image.set(dx + centre + 1, dy + y, colour);
            target.image.set(dx + left, dy + y + 1, colour);
            target.image.set(dx + centre + 2, dy + y + 1, colour);
        }
    }
}
