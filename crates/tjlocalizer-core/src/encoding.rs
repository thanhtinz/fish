//! Charset detection for resource text (specification §9).
//!
//! J2ME games predate any convention of declaring an encoding, so the bytes in a `.properties`
//! or `.dat` file could be anything. Guessing wrong does not fail loudly - it produces mojibake
//! that only shows up on the device - so every decode carries a confidence score and the ranked
//! alternatives, and the project profile can override the choice.

use encoding_rs::{Encoding, BIG5, GB18030, SHIFT_JIS, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252};
use serde::{Deserialize, Serialize};

/// One candidate decoding of a byte range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodingCandidate {
    pub label: String,
    /// 0.0 to 1.0. Not a probability, a ranking score.
    pub confidence: f32,
    pub had_errors: bool,
    /// The decoded text, so a caller can show a preview without decoding again.
    pub preview: String,
}

/// The charsets tried, in the order a J2ME game is likely to use them.
const CANDIDATES: &[&Encoding] = &[
    UTF_8,
    UTF_16LE,
    UTF_16BE,
    GB18030,
    BIG5,
    SHIFT_JIS,
    WINDOWS_1252,
];

/// Ranks the plausible decodings of `bytes`, best first.
///
/// Always returns at least one candidate: `WINDOWS_1252` maps every byte to some character, so
/// there is no input for which the tool can offer nothing at all.
pub fn detect(bytes: &[u8]) -> Vec<EncodingCandidate> {
    if let Some((encoding, _)) = Encoding::for_bom(bytes) {
        // A byte order mark is an explicit declaration; nothing heuristic beats it.
        let (text, _, had_errors) = encoding.decode(bytes);
        return vec![EncodingCandidate {
            label: encoding.name().to_string(),
            confidence: 1.0,
            had_errors,
            preview: preview_of(&text),
        }];
    }

    let mut out: Vec<EncodingCandidate> = CANDIDATES
        .iter()
        .map(|encoding| {
            let (text, _, had_errors) = encoding.decode(bytes);
            EncodingCandidate {
                label: encoding.name().to_string(),
                confidence: score(&text, had_errors, encoding),
                had_errors,
                preview: preview_of(&text),
            }
        })
        .collect();

    out.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    out
}

/// The best decoding, or `None` when nothing scores above `min_confidence`.
pub fn best(bytes: &[u8], min_confidence: f32) -> Option<EncodingCandidate> {
    detect(bytes)
        .into_iter()
        .find(|c| c.confidence >= min_confidence)
}

/// Scores a decoding on how much it looks like real text.
///
/// The signal that matters most is the *absence* of replacement characters and control bytes:
/// a wrong charset usually produces those in quantity. Printable ratio alone is not enough,
/// because WINDOWS_1252 decodes arbitrary bytes into printable accented letters and would
/// otherwise win against the correct multi-byte charset every time.
fn score(text: &str, had_errors: bool, encoding: &'static Encoding) -> f32 {
    if text.is_empty() {
        return 0.0;
    }

    let mut printable = 0usize;
    let mut controls = 0usize;
    let mut replacements = 0usize;
    let mut total = 0usize;

    for c in text.chars() {
        total += 1;
        if c == char::REPLACEMENT_CHARACTER {
            replacements += 1;
        } else if c == '\n' || c == '\r' || c == '\t' {
            printable += 1;
        } else if c.is_control() {
            controls += 1;
        } else {
            printable += 1;
        }
    }

    let total = total as f32;
    let mut confidence = printable as f32 / total;
    confidence -= (replacements as f32 / total) * 2.0;
    confidence -= (controls as f32 / total) * 1.5;

    if had_errors {
        confidence -= 0.25;
    }
    // Single-byte fallbacks decode anything, so they must not outrank a clean multi-byte read.
    if encoding == WINDOWS_1252 {
        confidence -= 0.20;
    }
    if encoding == UTF_8 && !had_errors {
        // Valid UTF-8 of any length is very unlikely to be an accident.
        confidence += 0.15;
    }

    confidence.clamp(0.0, 1.0)
}

fn preview_of(text: &str) -> String {
    text.chars().take(120).collect()
}

/// True when the bytes look like text rather than binary.
///
/// Used to decide whether a resource is worth extracting strings from at all. NUL bytes are the
/// giveaway: real text files in these games do not contain them, and binary blobs almost always
/// do.
pub fn looks_like_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let sample = &bytes[..bytes.len().min(4096)];
    if sample.contains(&0) {
        return false;
    }
    let printable = sample
        .iter()
        .filter(|&&b| b >= 0x20 || b == b'\n' || b == b'\r' || b == b'\t')
        .count();
    printable as f32 / sample.len() as f32 > 0.85
}
