//! Reading the words painted into artwork, in the game's own letters (§17).
//!
//! The rest of `assets` deliberately refuses to guess what an image says, and the reason given
//! was that a wrong reading is worse than none. That reason holds against *general* OCR - a model
//! trained on printed pages, asked about a twelve-pixel button, returning `5TART` with no way for
//! anyone to know. It does not hold against the reading done here.
//!
//! A game's button labels are drawn with the game's own font, and this project already has that
//! font: a glyph sheet, every letter in it, pixel for pixel. So the question is not "what letter
//! does this shape resemble" but "which of these ninety-five exact bitmaps is this shape" - and
//! that has an answer that can be checked. A glyph either matches the sheet's `S` to within a few
//! pixels or it does not, and when it does not, this module says so instead of picking the
//! nearest. Every character comes back with the score it matched at, a reading with any
//! unmatched glyph in it is not offered as text, and nothing is ever written into the project
//! without a person accepting it.
//!
//! What this cannot read: a logo lettered by hand, a label in a font the game does not ship,
//! anything scaled, rotated, outlined or shadowed past recognition. Those come back as unread,
//! which is the same answer this module gave before it could read anything at all.

use crate::font::sheet::{Image, Sheet};
use serde::{Deserialize, Serialize};

/// How well a shape must match a glyph before it is called that letter.
///
/// Intersection over union of the two ink masks. Identical bitmaps score 1.0; the same letter
/// resaved through a lossy step scores in the high eighties; a different letter of the same
/// width, `o` against `c` or `E` against `F`, lands well below this. Raising it further starts
/// refusing real matches, lowering it starts inventing them.
const MATCH: f32 = 0.74;

/// One character read out of an image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadGlyph {
    /// The letter it matched, or `None` where nothing in the sheet matched well enough.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter: Option<char>,
    /// How well it matched, 0 to 1.
    pub score: f32,
    /// Where the shape sits in the image, for a person checking the reading against the picture.
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// One line of writing found in an image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadLine {
    /// What the line says, with `\u{fffd}` standing in for each shape that matched nothing.
    pub text: String,
    pub glyphs: Vec<ReadGlyph>,
    pub top: u32,
    pub height: u32,
}

impl ReadLine {
    pub fn is_complete(&self) -> bool {
        !self.glyphs.is_empty() && self.glyphs.iter().all(|g| g.letter.is_some())
    }
}

/// What an image was found to say.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub entry: String,
    pub lines: Vec<ReadLine>,
    /// The worst glyph score in the reading. A reading is only as good as its weakest letter.
    pub confidence: f32,
    /// Shapes that matched nothing in the sheet.
    pub unread: usize,
}

impl Reading {
    /// Whether this reading is fit to offer as what the image says.
    ///
    /// Everything matched, and there was something to match. A reading that is *nearly* complete
    /// is not offered: `PLA?` is not a word, and a person shown it will accept it anyway.
    pub fn is_complete(&self) -> bool {
        self.unread == 0 && self.lines.iter().any(|l| !l.glyphs.is_empty())
    }

    /// The whole reading as text, lines joined by newlines.
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A glyph from the sheet, cropped to its ink and reduced to a mask.
struct Template {
    letter: char,
    mask: Mask,
}

/// A rectangle of ink-or-not.
struct Mask {
    width: u32,
    height: u32,
    bits: Vec<bool>,
}

impl Mask {
    fn get(&self, x: i64, y: i64) -> bool {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return false;
        }
        self.bits[(y as u32 * self.width + x as u32) as usize]
    }

    /// Intersection over union against another mask, at a small offset.
    ///
    /// The offset exists because a letter cropped out of a picture and the same letter cropped
    /// out of a sheet can disagree by a pixel about where their ink starts - an anti-aliased edge
    /// that survived one save and not the other. Without the offsets, a correct reading fails on
    /// a rounding difference; with more than one pixel of them, different letters start matching.
    fn overlap(&self, other: &Mask) -> f32 {
        let mut best = 0.0f32;
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let mut both = 0usize;
                let mut either = 0usize;
                let width = self.width.max(other.width) as i64 + 1;
                let height = self.height.max(other.height) as i64 + 1;
                for y in -1..height {
                    for x in -1..width {
                        let a = self.get(x, y);
                        let b = other.get(x - dx, y - dy);
                        if a && b {
                            both += 1;
                        }
                        if a || b {
                            either += 1;
                        }
                    }
                }
                if either > 0 {
                    best = best.max(both as f32 / either as f32);
                }
            }
        }
        best
    }
}

/// The letters a sheet can be asked about, as masks.
fn templates(sheet: &Sheet) -> Vec<Template> {
    let mut out = Vec::new();
    for letter in &sheet.order {
        if letter.is_whitespace() {
            continue;
        }
        let Some(bounds) = sheet.ink_bounds(*letter) else {
            continue;
        };
        let (ox, oy) = sheet.grid.cell_origin(bounds.cell);
        let mut bits = Vec::with_capacity((bounds.width * bounds.height) as usize);
        for y in 0..bounds.height {
            for x in 0..bounds.width {
                let pixel = sheet.image.get(ox + bounds.x + x, oy + bounds.y + y);
                bits.push(is_ink(pixel, sheet.background));
            }
        }
        out.push(Template {
            letter: *letter,
            mask: Mask {
                width: bounds.width,
                height: bounds.height,
                bits,
            },
        });
    }
    out
}

/// Whether a pixel is ink rather than the background it was drawn on.
fn is_ink(colour: [u8; 4], background: [u8; 4]) -> bool {
    if colour[3] < 24 {
        return false;
    }
    if background[3] == 0 {
        return true;
    }
    (0..3).any(|i| colour[i].abs_diff(background[i]) > 24)
}

/// What an image treats as nothing.
///
/// A sheet is told its background; an image found in a game archive is not, so it has to be
/// worked out. Anything transparent means transparency is the background; otherwise the colour
/// covering most of the border is - a label is drawn on something, and that something reaches the
/// edges.
fn background_of(image: &Image) -> [u8; 4] {
    for y in 0..image.height {
        for x in 0..image.width {
            if image.get(x, y)[3] < 24 {
                return [0, 0, 0, 0];
            }
        }
    }

    let mut counts: std::collections::HashMap<[u8; 4], usize> = std::collections::HashMap::new();
    for x in 0..image.width {
        *counts.entry(image.get(x, 0)).or_default() += 1;
        *counts
            .entry(image.get(x, image.height.saturating_sub(1)))
            .or_default() += 1;
    }
    for y in 0..image.height {
        *counts.entry(image.get(0, y)).or_default() += 1;
        *counts
            .entry(image.get(image.width.saturating_sub(1), y))
            .or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(colour, _)| colour)
        .unwrap_or([0, 0, 0, 0])
}

/// Read an image with the letters of one sheet.
pub fn read(entry: &str, image: &Image, sheet: &Sheet) -> Reading {
    let templates = templates(sheet);
    let background = background_of(image);

    let mut mask = Mask {
        width: image.width,
        height: image.height,
        bits: Vec::with_capacity((image.width * image.height) as usize),
    };
    for y in 0..image.height {
        for x in 0..image.width {
            mask.bits.push(is_ink(image.get(x, y), background));
        }
    }

    let mut lines = Vec::new();
    let mut unread = 0usize;
    let mut confidence = 1.0f32;

    for (top, bottom) in bands(&mask) {
        let line = read_line(&mask, top, bottom, &templates);
        unread += line.glyphs.iter().filter(|g| g.letter.is_none()).count();
        for glyph in &line.glyphs {
            confidence = confidence.min(glyph.score);
        }
        lines.push(line);
    }

    if lines.iter().all(|l| l.glyphs.is_empty()) {
        confidence = 0.0;
    }

    Reading {
        entry: entry.to_string(),
        lines,
        confidence,
        unread,
    }
}

/// Rows of ink separated by rows without, which is what lines of writing are.
fn bands(mask: &Mask) -> Vec<(u32, u32)> {
    let mut bands = Vec::new();
    let mut start = None;
    for y in 0..mask.height {
        let has_ink = (0..mask.width).any(|x| mask.get(x as i64, y as i64));
        match (has_ink, start) {
            (true, None) => start = Some(y),
            (false, Some(from)) => {
                bands.push((from, y - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        bands.push((from, mask.height - 1));
    }
    bands
}

fn read_line(mask: &Mask, top: u32, bottom: u32, templates: &[Template]) -> ReadLine {
    let height = bottom - top + 1;

    // Columns of ink, which for a bitmap font are letters: the sheets a game ships leave a pixel
    // between glyphs, because the game draws them side by side and would otherwise smudge.
    let mut blobs: Vec<(u32, u32)> = Vec::new();
    let mut start = None;
    for x in 0..mask.width {
        let has_ink = (top..=bottom).any(|y| mask.get(x as i64, y as i64));
        match (has_ink, start) {
            (true, None) => start = Some(x),
            (false, Some(from)) => {
                blobs.push((from, x - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        blobs.push((from, mask.width - 1));
    }

    let widest = templates.iter().map(|t| t.mask.width).max().unwrap_or(1);

    let mut glyphs: Vec<ReadGlyph> = Vec::new();
    for (left, right) in &blobs {
        // Letters that touch - kerned tightly, or joined by an anti-aliased pixel - arrive as one
        // blob. Splitting is tried only when the blob is too wide to be a single letter, so a
        // clean `W` is never sawn in half looking for two narrower letters.
        if right - left + 1 > widest {
            glyphs.extend(split(mask, *left, *right, top, bottom, templates, 0));
        } else {
            glyphs.push(match_blob(mask, *left, *right, top, bottom, templates));
        }
    }

    // A space is a gap noticeably wider than the gaps between the letters of a word, and both
    // measurements have to come off this line: a game draws its letters on its own pitch, and a
    // fixed threshold reads a wide-tracked title as one word per letter or a tight one as no
    // words at all.
    let mut gaps: Vec<u32> = Vec::new();
    for pair in glyphs.windows(2) {
        gaps.push(pair[1].x.saturating_sub(pair[0].x + pair[0].width));
    }
    let mut sorted = gaps.clone();
    sorted.sort_unstable();
    let between = sorted.get(sorted.len() / 2).copied().unwrap_or(0);
    let typical = if glyphs.is_empty() {
        1
    } else {
        glyphs.iter().map(|g| g.width).sum::<u32>() / glyphs.len() as u32
    };
    // The margin is most of a letter's width: a word's letters vary in how tightly they sit
    // against each other by a pixel or two, and a space is a whole missing letter.
    let space = between + (typical * 3 / 4).max(3);

    let mut text = String::new();
    let mut previous_end: Option<u32> = None;
    for glyph in &glyphs {
        if let Some(end) = previous_end {
            if glyph.x.saturating_sub(end) >= space {
                text.push(' ');
            }
        }
        text.push(glyph.letter.unwrap_or('\u{fffd}'));
        previous_end = Some(glyph.x + glyph.width);
    }

    ReadLine {
        text,
        glyphs,
        top,
        height,
    }
}

/// The best letter for one blob of ink, cropped to what it actually covers.
fn match_blob(
    mask: &Mask,
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
    templates: &[Template],
) -> ReadGlyph {
    let (crop, y) = crop(mask, left, right, top, bottom);
    let mut best: Option<(&Template, f32)> = None;
    for template in templates {
        // Sizes that cannot be the same letter are not compared: it is the bulk of the work and
        // the answer is known.
        if template.mask.width.abs_diff(crop.width) > 1
            || template.mask.height.abs_diff(crop.height) > 1
        {
            continue;
        }
        let score = crop.overlap(&template.mask);
        if best.map(|(_, b)| score > b).unwrap_or(true) {
            best = Some((template, score));
        }
    }

    let (letter, score) = match best {
        Some((template, score)) if score >= MATCH => (Some(template.letter), score),
        Some((_, score)) => (None, score),
        None => (None, 0.0),
    };

    ReadGlyph {
        letter,
        score,
        x: left,
        y,
        width: crop.width,
        height: crop.height,
    }
}

/// Split a blob too wide to be one letter, at whichever cut reads best.
fn split(
    mask: &Mask,
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
    templates: &[Template],
    depth: u32,
) -> Vec<ReadGlyph> {
    let whole = match_blob(mask, left, right, top, bottom, templates);
    if whole.letter.is_some() || depth >= 8 || right <= left {
        return vec![whole];
    }

    let narrowest = templates.iter().map(|t| t.mask.width).min().unwrap_or(1);
    let mut best: Option<(f32, Vec<ReadGlyph>)> = None;
    for cut in (left + narrowest.max(1) - 1)..right {
        let head = match_blob(mask, left, cut, top, bottom, templates);
        if head.letter.is_none() {
            continue;
        }
        let mut glyphs = vec![head];
        glyphs.extend(split(
            mask,
            cut + 1,
            right,
            top,
            bottom,
            templates,
            depth + 1,
        ));
        let worst = glyphs.iter().map(|g| g.score).fold(1.0f32, f32::min);
        let complete = glyphs.iter().all(|g| g.letter.is_some());
        let ranked = if complete { worst } else { worst / 2.0 };
        if best.as_ref().map(|(b, _)| ranked > *b).unwrap_or(true) {
            best = Some((ranked, glyphs));
        }
    }

    match best {
        Some((_, glyphs)) if glyphs.iter().all(|g| g.letter.is_some()) => glyphs,
        _ => vec![whole],
    }
}

/// A blob's ink, cropped tight, with the row it starts on.
fn crop(mask: &Mask, left: u32, right: u32, top: u32, bottom: u32) -> (Mask, u32) {
    let mut first = bottom;
    let mut last = top;
    let mut found = false;
    for y in top..=bottom {
        for x in left..=right {
            if mask.get(x as i64, y as i64) {
                found = true;
                first = first.min(y);
                last = last.max(y);
            }
        }
    }
    if !found {
        return (
            Mask {
                width: 0,
                height: 0,
                bits: Vec::new(),
            },
            top,
        );
    }

    let width = right - left + 1;
    let height = last - first + 1;
    let mut bits = Vec::with_capacity((width * height) as usize);
    for y in first..=last {
        for x in left..=right {
            bits.push(mask.get(x as i64, y as i64));
        }
    }
    (
        Mask {
            width,
            height,
            bits,
        },
        first,
    )
}
