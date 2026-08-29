//! Images with words painted into them (§17).
//!
//! A game's text is not all in its constant pool. Buttons, logos and banners are often artwork
//! with the words already drawn on, and no amount of translating strings touches them. A build
//! can be reported as fully translated, pass every check in this project, and still show a player
//! an English START button.
//!
//! That blind spot is what this module is about. It cannot read the words - there is no OCR here,
//! and a wrong reading would be worse than none - so it does not pretend to. It reports the shape
//! of each image and what about that shape resembles a label, a person decides, and the decision
//! is recorded so the rest of the tool can hold the project to it: an image marked as carrying
//! text is a piece of unfinished work until something replaces it.

use crate::jar::Archive;
use crate::Result;
use serde::{Deserialize, Serialize};

/// One image in the game, and what can be said about it without reading it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAsset {
    pub entry: String,
    pub width: u32,
    pub height: u32,
    /// How many distinct colours. Artwork has thousands; a button label has a handful.
    pub colours: usize,
    /// Share of pixels carrying ink.
    pub ink_share: f32,
    /// Horizontal bands of ink separated by empty rows. A line of text is one band with clear
    /// space above and below it; a photograph has none.
    pub bands: usize,
    /// What about this image resembles a label. Evidence, not a verdict: every one of these is
    /// something a person can check by looking at the picture.
    ///
    /// Facts rather than sentences, because two interfaces have to say them and they do not speak
    /// the same language: the command line is English and the application is Vietnamese. A core
    /// that hands out finished prose forces one of them to show the other's wording.
    pub hints: Vec<Hint>,
}

/// One reason an image might have words painted into it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// `rename_all` on an enum renames the variants; the fields inside them need saying separately, or
// they reach an interface under their Rust names and read as missing.
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Hint {
    /// Whoever drew the game called the file something. The weakest evidence, and often right.
    NameSuggests { word: String },
    /// Few colours over little of the image: lettering, not a scene.
    FewColours { colours: usize, ink_percent: u32 },
    /// A wide, short image with one to three bands of ink: the shape of a line of writing.
    ShapeOfALine {
        width: u32,
        height: u32,
        bands: usize,
    },
}

impl ImageAsset {
    /// Whether anything about this image suggests words are painted into it.
    ///
    /// Deliberately not a probability. A number invites being treated as an answer, and this is
    /// not an answer - it is a reason to look.
    pub fn worth_checking(&self) -> bool {
        !self.hints.is_empty()
    }
}

/// Every image in the archive, with what its shape suggests.
pub fn scan(archive: &Archive) -> Result<Vec<ImageAsset>> {
    let mut found = Vec::new();
    for entry in archive.entries() {
        if entry.extension() != "png" {
            continue;
        }
        // An image that will not decode is not something to stop for: the archive is somebody
        // else's and may hold anything.
        if let Ok(asset) = inspect(&entry.name, &entry.data) {
            found.push(asset);
        }
    }
    found.sort_by(|a, b| {
        b.hints
            .len()
            .cmp(&a.hints.len())
            .then(a.entry.cmp(&b.entry))
    });
    Ok(found)
}

fn inspect(entry: &str, bytes: &[u8]) -> Result<ImageAsset> {
    let image = crate::font::sheet::Image::decode_png(bytes)?;

    let mut colours = std::collections::HashSet::new();
    let mut inked = 0usize;
    let mut row_has_ink = vec![false; image.height as usize];

    for y in 0..image.height {
        for x in 0..image.width {
            let pixel = image.get(x, y);
            if pixel[3] >= 24 {
                inked += 1;
                row_has_ink[y as usize] = true;
                if colours.len() <= 4096 {
                    colours.insert(pixel);
                }
            }
        }
    }

    let mut bands = 0usize;
    let mut inside = false;
    for has_ink in &row_has_ink {
        if *has_ink && !inside {
            bands += 1;
        }
        inside = *has_ink;
    }

    let total = (image.width * image.height).max(1) as f32;
    let ink_share = inked as f32 / total;
    let mut asset = ImageAsset {
        entry: entry.to_string(),
        width: image.width,
        height: image.height,
        colours: colours.len(),
        ink_share,
        bands,
        hints: Vec::new(),
    };
    asset.hints = hints(&asset);
    Ok(asset)
}

/// What about an image's shape resembles a label.
fn hints(asset: &ImageAsset) -> Vec<Hint> {
    let mut hints = Vec::new();
    let name = asset.entry.to_lowercase();

    // A name is the weakest evidence and the most often right: whoever drew the game called this
    // file something.
    const NAMES: [&str; 12] = [
        "btn", "button", "menu", "label", "text", "title", "logo", "banner", "word", "start",
        "help", "about",
    ];
    if let Some(word) = NAMES.iter().find(|w| name.contains(*w)) {
        hints.push(Hint::NameSuggests {
            word: (*word).to_string(),
        });
    }

    // Few colours and little ink: lettering, not a scene.
    if asset.colours <= 16 && asset.ink_share < 0.5 {
        hints.push(Hint::FewColours {
            colours: asset.colours,
            ink_percent: (asset.ink_share * 100.0).round() as u32,
        });
    }

    // One to three bands of ink in a wide, short image is the shape of a line of writing.
    let wide = asset.width >= asset.height * 2;
    if wide && (1..=3).contains(&asset.bands) && asset.height <= 64 {
        hints.push(Hint::ShapeOfALine {
            width: asset.width,
            height: asset.height,
            bands: asset.bands,
        });
    }
    hints
}

/// An image somebody has looked at and decided carries words.
///
/// The point of writing it down is that the tool can then hold the project to it. An image nobody
/// has decided about is a question; an image marked as carrying text and not replaced is a piece
/// of the translation that is not done.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextAsset {
    pub entry: String,
    /// What it says, as far as a person could tell. Free text, for the person who will redraw it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub says: String,
    /// A redrawn version, relative to the project directory. Absent means still to do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
}
