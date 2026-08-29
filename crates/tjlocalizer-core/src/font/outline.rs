//! Diacritics taken from a real typeface (specification §16).
//!
//! Composing a glyph needs two things: the game's own letter, and a mark to put over it. The
//! letter has to come from the game or the result does not belong. The *mark* does not - a tone
//! mark is the same shape in every typeface, and drawing it by hand in four pixels gives a blunt
//! approximation of one.
//!
//! So a font file can be pointed at, and the mark is lifted from it: rasterise the precomposed
//! letter and its base at a size where the base matches the game's letter, and the mark is what
//! the first has that the second does not. Any font with Vietnamese letters serves, which is
//! nearly every font shipped in Vietnam.
//!
//! The font is read from wherever the user keeps it and never copied into the project. A font is
//! somebody's work under somebody's licence, and a localization tool has no business
//! redistributing one.

use crate::font::Composition;
use crate::Result;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};

/// A rasterised mark, positioned relative to the base letter's ink box.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    /// One byte of coverage per pixel, row-major.
    pub coverage: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Where to put it, measured from the base letter's ink box: `dx` from its left edge, `dy`
    /// from its top. Negative `dy` is above the letter, which is where most marks go.
    pub dx: i32,
    pub dy: i32,
}

impl Mark {
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0 || self.coverage.iter().all(|c| *c < 96)
    }
}

/// A typeface to take marks from.
pub struct MarkSource {
    font: FontVec,
    /// Where it came from, for a report. The bytes are never written anywhere.
    pub name: String,
}

impl MarkSource {
    pub fn load(bytes: Vec<u8>, name: impl Into<String>) -> Result<Self> {
        let font = FontVec::try_from_vec(bytes).map_err(|e| crate::Error::InvalidProject {
            path: std::path::PathBuf::from("<font>"),
            reason: format!("this is not a font this build can read: {e}"),
        })?;
        Ok(Self {
            font,
            name: name.into(),
        })
    }

    pub fn from_path(path: &std::path::Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::load(bytes, path.display().to_string())
    }

    /// Whether the typeface can draw a character at all.
    pub fn has(&self, c: char) -> bool {
        self.font.glyph_id(c).0 != 0
    }

    /// Whether it covers enough of Vietnamese to be worth using.
    pub fn covers_vietnamese(&self) -> bool {
        crate::font::vietnamese_required()
            .iter()
            .all(|c| self.has(*c))
    }

    /// The mark that turns `base` into `composed`, sized so the base matches a letter this tall.
    ///
    /// Returns `None` when the typeface lacks either character, or when the difference is empty -
    /// which happens if the font draws the composed letter as a single outline that shares no
    /// pixels with the base, and means this font cannot be used for that letter.
    pub fn mark_for(&self, composition: &Composition, base_ink_height: u32) -> Option<Mark> {
        if base_ink_height == 0 || !self.has(composition.base) || !self.has(composition.composed) {
            return None;
        }

        // Chosen so the *base letter* comes out the height of the game's letter. Scaling by the
        // composed letter instead would shrink the base to make room for the mark, and the mark
        // would then be sized for a letter that is not the one it is going over.
        let scale = self.scale_for(composition.base, base_ink_height)?;
        let base = self.raster(composition.base, scale)?;
        let composed = self.raster(composition.composed, scale)?;

        // Both are positioned from the same origin, so subtracting works directly.
        let min_x = base.left.min(composed.left);
        let min_y = base.top.min(composed.top);
        let max_x = (base.left + base.width as i32).max(composed.left + composed.width as i32);
        let max_y = (base.top + base.height as i32).max(composed.top + composed.height as i32);

        let width = (max_x - min_x).max(0) as u32;
        let height = (max_y - min_y).max(0) as u32;
        if width == 0 || height == 0 {
            return None;
        }

        let mut difference = vec![0u8; (width * height) as usize];
        for y in 0..height {
            for x in 0..width {
                let gx = min_x + x as i32;
                let gy = min_y + y as i32;
                let c = composed.at(gx, gy);
                let b = base.at(gx, gy);
                difference[(y * width + x) as usize] = c.saturating_sub(b);
            }
        }

        // Trimmed to the ink, so the offsets mean something.
        let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0u32, 0u32);
        let mut any = false;
        for y in 0..height {
            for x in 0..width {
                if difference[(y * width + x) as usize] >= 96 {
                    any = true;
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x);
                    bottom = bottom.max(y);
                }
            }
        }
        if !any {
            return None;
        }

        let (mw, mh) = (right - left + 1, bottom - top + 1);
        let mut coverage = vec![0u8; (mw * mh) as usize];
        for y in 0..mh {
            for x in 0..mw {
                coverage[(y * mw + x) as usize] =
                    difference[((y + top) * width + (x + left)) as usize];
            }
        }

        Some(Mark {
            coverage,
            width: mw,
            height: mh,
            // Relative to the base letter's ink box, which is what the caller knows about.
            dx: (min_x + left as i32) - base.left,
            dy: (min_y + top as i32) - base.top,
        })
    }

    /// The pixel scale at which a character's ink is `wanted` pixels tall.
    fn scale_for(&self, c: char, wanted: u32) -> Option<PxScale> {
        // Measured rather than derived from the font's metrics: what matters is the ink, and the
        // relation between ink height and em size differs per typeface.
        let probe = 64.0f32;
        let measured = self.raster(c, PxScale::from(probe))?.height;
        if measured == 0 {
            return None;
        }
        Some(PxScale::from(probe * wanted as f32 / measured as f32))
    }

    fn raster(&self, c: char, scale: PxScale) -> Option<Raster> {
        let scaled = self.font.as_scaled(scale);
        let glyph = self
            .font
            .glyph_id(c)
            .with_scale_and_position(scale, ab_glyph::point(0.0, scaled.ascent()));
        let outlined = self.font.outline_glyph(glyph)?;
        let bounds = outlined.px_bounds();

        let width = bounds.width().ceil().max(0.0) as u32;
        let height = bounds.height().ceil().max(0.0) as u32;
        if width == 0 || height == 0 {
            return None;
        }
        let mut coverage = vec![0u8; (width * height) as usize];
        outlined.draw(|x, y, c| {
            if x < width && y < height {
                coverage[(y * width + x) as usize] = (c * 255.0).clamp(0.0, 255.0) as u8;
            }
        });

        Some(Raster {
            coverage,
            width,
            height,
            left: bounds.min.x.floor() as i32,
            top: bounds.min.y.floor() as i32,
        })
    }
}

/// One rasterised glyph, positioned in the font's own pixel space.
struct Raster {
    coverage: Vec<u8>,
    width: u32,
    height: u32,
    left: i32,
    top: i32,
}

impl Raster {
    fn at(&self, x: i32, y: i32) -> u8 {
        let (lx, ly) = (x - self.left, y - self.top);
        if lx < 0 || ly < 0 || lx as u32 >= self.width || ly as u32 >= self.height {
            return 0;
        }
        self.coverage[(ly as u32 * self.width + lx as u32) as usize]
    }
}
