//! What a font can draw, and what Vietnamese needs it to (specification §16).
//!
//! This is the problem that stops a J2ME localization dead. A game that draws its text with a
//! bitmap font can only show the characters someone drew into its glyph sheet, and nobody drew
//! `ế`. The translation is correct, the build validates, the game runs - and the screen shows
//! blanks. Nothing else in the pipeline can see that, because by every other measure the text is
//! fine.
//!
//! Vietnamese is unusually demanding here: 134 letters beyond ASCII, because every vowel takes a
//! modifier, a tone, or both. A font that covers French or German is nowhere near enough.
//!
//! This module holds the language facts and the coverage arithmetic. Reading and writing an
//! actual glyph sheet is `font::sheet`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub mod sheet;

/// A modification to the vowel itself, not a tone.
///
/// These change which letter it is - `a` and `ă` are different vowels in Vietnamese, not the same
/// vowel said differently - so they must be drawn even in text with no tones at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VowelMark {
    /// ă - a cup above.
    Breve,
    /// â ê ô - a caret above.
    Circumflex,
    /// ơ ư - a hook at the upper right.
    Horn,
    /// đ - a bar through the ascender.
    Stroke,
}

/// One of the six tones. The level tone is unmarked, so five marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// á - sắc
    Acute,
    /// à - huyền
    Grave,
    /// ả - hỏi
    HookAbove,
    /// ã - ngã
    Tilde,
    /// ạ - nặng, the only one drawn below
    DotBelow,
}

impl Tone {
    /// Whether the mark sits under the letter rather than over it.
    pub fn is_below(self) -> bool {
        self == Tone::DotBelow
    }

    pub fn all() -> [Tone; 5] {
        [
            Tone::Acute,
            Tone::Grave,
            Tone::HookAbove,
            Tone::Tilde,
            Tone::DotBelow,
        ]
    }
}

/// How one Vietnamese letter is built from a letter a font already has.
///
/// The base is always an ASCII letter, which is the point: a bitmap font drawn for a game has its
/// own weight, its own pixel grid and its own personality, and a glyph composed from that game's
/// own `a` looks like it belongs. A glyph taken from somewhere else does not.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Composition {
    pub composed: char,
    pub base: char,
    pub vowel_mark: Option<VowelMark>,
    pub tone: Option<Tone>,
}

/// Every letter Vietnamese needs beyond ASCII, and how to build each one.
///
/// Built from the twelve vowels rather than written out, so a missing letter is impossible - the
/// list in the specification is 134 characters long and a hand-written table would have a typo in
/// it that nobody found until a game shipped with one letter blank.
pub fn vietnamese_compositions() -> Vec<Composition> {
    // base letter, the modification that makes this vowel, and its uppercase form.
    const VOWELS: &[(char, char, Option<VowelMark>)] = &[
        ('a', 'A', None),
        ('a', 'A', Some(VowelMark::Breve)),
        ('a', 'A', Some(VowelMark::Circumflex)),
        ('e', 'E', None),
        ('e', 'E', Some(VowelMark::Circumflex)),
        ('i', 'I', None),
        ('o', 'O', None),
        ('o', 'O', Some(VowelMark::Circumflex)),
        ('o', 'O', Some(VowelMark::Horn)),
        ('u', 'U', None),
        ('u', 'U', Some(VowelMark::Horn)),
        ('y', 'Y', None),
    ];

    let mut out = Vec::new();

    for (lower, upper, mark) in VOWELS {
        // The unmarked, untoned form is ASCII and needs no glyph; a modified one does.
        if mark.is_some() {
            for (base, _) in [(*lower, false), (*upper, true)] {
                if let Some(composed) = compose(base, *mark, None) {
                    out.push(Composition {
                        composed,
                        base,
                        vowel_mark: *mark,
                        tone: None,
                    });
                }
            }
        }
        for tone in Tone::all() {
            for base in [*lower, *upper] {
                if let Some(composed) = compose(base, *mark, Some(tone)) {
                    out.push(Composition {
                        composed,
                        base,
                        vowel_mark: *mark,
                        tone: Some(tone),
                    });
                }
            }
        }
    }

    // đ is a consonant and takes no tone, so it sits outside the vowel loop.
    for base in ['d', 'D'] {
        if let Some(composed) = compose(base, Some(VowelMark::Stroke), None) {
            out.push(Composition {
                composed,
                base,
                vowel_mark: Some(VowelMark::Stroke),
                tone: None,
            });
        }
    }

    out
}

/// The precomposed character for a base letter with a modification and a tone.
///
/// A table rather than Unicode normalisation: the crate that would do this properly is a large
/// dependency for one language's alphabet, and the alphabet does not change.
fn compose(base: char, mark: Option<VowelMark>, tone: Option<Tone>) -> Option<char> {
    let row: &[char] = match (base.to_ascii_lowercase(), mark) {
        //            plain  acute grave hook  tilde dot
        ('a', None) => &['a', 'á', 'à', 'ả', 'ã', 'ạ'],
        ('a', Some(VowelMark::Breve)) => &['ă', 'ắ', 'ằ', 'ẳ', 'ẵ', 'ặ'],
        ('a', Some(VowelMark::Circumflex)) => &['â', 'ấ', 'ầ', 'ẩ', 'ẫ', 'ậ'],
        ('e', None) => &['e', 'é', 'è', 'ẻ', 'ẽ', 'ẹ'],
        ('e', Some(VowelMark::Circumflex)) => &['ê', 'ế', 'ề', 'ể', 'ễ', 'ệ'],
        ('i', None) => &['i', 'í', 'ì', 'ỉ', 'ĩ', 'ị'],
        ('o', None) => &['o', 'ó', 'ò', 'ỏ', 'õ', 'ọ'],
        ('o', Some(VowelMark::Circumflex)) => &['ô', 'ố', 'ồ', 'ổ', 'ỗ', 'ộ'],
        ('o', Some(VowelMark::Horn)) => &['ơ', 'ớ', 'ờ', 'ở', 'ỡ', 'ợ'],
        ('u', None) => &['u', 'ú', 'ù', 'ủ', 'ũ', 'ụ'],
        ('u', Some(VowelMark::Horn)) => &['ư', 'ứ', 'ừ', 'ử', 'ữ', 'ự'],
        ('y', None) => &['y', 'ý', 'ỳ', 'ỷ', 'ỹ', 'ỵ'],
        ('d', Some(VowelMark::Stroke)) => &['đ'],
        _ => return None,
    };

    let index = match tone {
        None => 0,
        Some(Tone::Acute) => 1,
        Some(Tone::Grave) => 2,
        Some(Tone::HookAbove) => 3,
        Some(Tone::Tilde) => 4,
        Some(Tone::DotBelow) => 5,
    };
    let composed = *row.get(index)?;

    Some(if base.is_uppercase() {
        // Vietnamese uppercase forms all exist and are one-to-one with the lowercase ones.
        composed.to_uppercase().next().unwrap_or(composed)
    } else {
        composed
    })
}

/// Every character Vietnamese text can contain beyond ASCII.
pub fn vietnamese_required() -> BTreeSet<char> {
    vietnamese_compositions()
        .iter()
        .map(|c| c.composed)
        .collect()
}

/// What a font can draw.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Coverage {
    /// The codepoints the font has a glyph for.
    pub covered: BTreeSet<char>,
    /// Where this was determined from, for a report.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
}

impl Coverage {
    pub fn new(covered: impl IntoIterator<Item = char>, source: impl Into<String>) -> Self {
        Self {
            covered: covered.into_iter().collect(),
            source: source.into(),
        }
    }

    /// A font that draws printable ASCII and nothing else - what a J2ME game's own sheet usually
    /// is, and the case this whole module exists for.
    pub fn ascii(source: impl Into<String>) -> Self {
        Self::new((0x20u8..=0x7E).map(|b| b as char), source)
    }

    pub fn covers(&self, c: char) -> bool {
        // Whitespace and control characters are laid out rather than drawn, so a font that has no
        // glyph for a space is not missing one.
        c.is_whitespace() || c.is_control() || self.covered.contains(&c)
    }

    /// The characters in this text the font cannot draw, in order, without repeats.
    pub fn missing_in(&self, text: &str) -> Vec<char> {
        let mut seen = BTreeSet::new();
        text.chars()
            .filter(|c| !self.covers(*c))
            .filter(|c| seen.insert(*c))
            .collect()
    }

    /// What Vietnamese needs and this font does not have.
    pub fn missing_for_vietnamese(&self) -> Vec<char> {
        vietnamese_required()
            .into_iter()
            .filter(|c| !self.covers(*c))
            .collect()
    }

    /// Which of the missing Vietnamese letters could be built from letters this font does have.
    ///
    /// The useful number: a font with the ASCII alphabet can have all 134 composed for it, and one
    /// missing its base letters cannot.
    pub fn composable(&self) -> Vec<Composition> {
        vietnamese_compositions()
            .into_iter()
            .filter(|c| !self.covers(c.composed) && self.covers(c.base))
            .collect()
    }
}

/// What a font can and cannot do for a body of text.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageReport {
    pub source: String,
    pub covered_count: usize,
    /// Characters used by the translations that the font cannot draw.
    pub missing_used: Vec<char>,
    /// How many strings are affected.
    pub affected_strings: usize,
    /// Vietnamese letters absent from the font, whether or not the translations use them yet.
    pub missing_required: Vec<char>,
    /// Of those, the ones that could be built from letters the font already has.
    pub composable_count: usize,
}

/// Checks a set of translations against a font.
///
/// Takes the strings rather than the project so it can be used on a draft, on one language, or on
/// a single line in the interface.
pub fn report<'a>(
    coverage: &Coverage,
    strings: impl IntoIterator<Item = &'a str>,
) -> CoverageReport {
    let mut missing_used = BTreeSet::new();
    let mut affected = 0usize;

    for text in strings {
        let missing = coverage.missing_in(text);
        if !missing.is_empty() {
            affected += 1;
            missing_used.extend(missing);
        }
    }

    CoverageReport {
        source: coverage.source.clone(),
        covered_count: coverage.covered.len(),
        missing_used: missing_used.into_iter().collect(),
        affected_strings: affected,
        missing_required: coverage.missing_for_vietnamese(),
        composable_count: coverage.composable().len(),
    }
}
