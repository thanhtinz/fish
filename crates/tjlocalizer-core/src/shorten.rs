//! Ways to make a label fit, when the measurement says it will not (§24).
//!
//! The layout check reports that a translation draws wider than the original, and the proof sheet
//! shows it. Neither helps anybody fix it, and the fix is the part a translator actually spends
//! time on: finding a shorter way to say the same thing that is still Vietnamese and still means
//! what the game meant.
//!
//! Nothing here invents wording. Every alternative comes from something the project already
//! holds - a second reading in its dictionary, a word its own register profile says to drop - and
//! carries the reason with it, because a suggestion a translator cannot check is a suggestion
//! they have to re-derive. Nothing is applied: these are offered, measured, and chosen by a
//! person.

use crate::dictionary::{Dictionary, Segment};
use crate::font::metrics::Metrics;
use crate::lang::Language;
use crate::register;

/// A shorter way of saying the same thing, and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Alternative {
    pub text: String,
    /// Width in the game's own pixels, when the game has a sheet to measure with.
    pub width: Option<u32>,
    /// Why this is being offered, in one line a translator can judge.
    pub why: String,
}

/// Shorter renderings of `current`, narrowest first.
///
/// `current` is the translation as it stands, not the source: the alternatives are edits to it,
/// so that everything a translator already decided about the rest of the line survives.
pub fn alternatives(
    source: &str,
    current: &str,
    dictionary: &Dictionary,
    from: &Language,
    to: &Language,
    context: &str,
    metrics: Option<&Metrics>,
) -> Vec<Alternative> {
    let mut found: Vec<Alternative> = Vec::new();

    from_other_readings(source, current, dictionary, from, to, context, &mut found);
    from_dropped_words(current, to, &mut found);

    // Measured before filtering, not after: the filter is "is this actually narrower", and it
    // has to be asked in pixels wherever pixels are available. Filtering first and measuring
    // afterwards would rank on character counts and then print widths that contradict the
    // ranking.
    for alternative in &mut found {
        alternative.width = width_of(&alternative.text, metrics);
    }
    let baseline = width_of(current, metrics);
    found.retain(|a| a.text != current && shorter(a.width, baseline, &a.text, current));
    found.sort_by(|a, b| match (a.width, b.width) {
        (Some(x), Some(y)) => x.cmp(&y),
        _ => a.text.chars().count().cmp(&b.text.chars().count()),
    });
    found.dedup_by(|a, b| a.text == b.text);
    found
}

fn width_of(text: &str, metrics: Option<&Metrics>) -> Option<u32> {
    metrics.and_then(|m| m.measure(text))
}

fn shorter(candidate: Option<u32>, baseline: Option<u32>, text: &str, current: &str) -> bool {
    match (candidate, baseline) {
        (Some(a), Some(b)) => a < b,
        // With no sheet to measure against, characters are what is left. It is a worse question,
        // and saying so is better than declining to help at all.
        _ => text.chars().count() < current.chars().count(),
    }
}

/// Substitutes another of the dictionary's readings for a term the translation used.
///
/// Which reading the translation used is found by looking, not by asking the dictionary which one
/// it would have picked. Those are different questions: a translator may have taken the second
/// reading deliberately, and a suggestion list built on the assumption they took the first would
/// have nothing to say to them.
fn from_other_readings(
    source: &str,
    current: &str,
    dictionary: &Dictionary,
    from: &Language,
    to: &Language,
    context: &str,
    out: &mut Vec<Alternative>,
) {
    for segment in dictionary.segment(source, from, to, context) {
        let Segment::Term { text, .. } = segment else {
            continue;
        };
        let readings = dictionary.readings(&text, from, to, context);

        for used in &readings {
            // Only where the translation actually says this. Substituting into a line that says
            // something else would rewrite a translator's own wording behind their back.
            if !contains_word(current, &used.target) {
                continue;
            }
            for other in &readings {
                if other.target == used.target {
                    continue;
                }
                let replaced = replace_whole_words(current, &used.target, &other.target);
                if replaced == current {
                    continue;
                }
                let mut why = format!("{text} → {} thay cho {}", other.target, used.target);
                if !other.note.is_empty() {
                    why.push_str(&format!(" ({})", other.note));
                }
                out.push(Alternative {
                    text: replaced,
                    width: None,
                    why,
                });
            }
        }
    }
}

/// Replaces a term only where it stands as a word of its own, ignoring case.
///
/// Two things this has to get right, both found by running it against real rows rather than
/// fixtures. Word boundaries: "bắt" sits inside "bắt đầu", and a plain replace would cut a word
/// in half and offer the result as an improvement. And case: a dictionary keeps its readings in
/// lower case, while a label in a game is capitalised, so an exact match finds nothing and the
/// whole feature silently offers nothing at all. The replacement takes the capitalisation of the
/// text it replaces.
fn replace_whole_words(text: &str, from: &str, to: &str) -> String {
    let haystack: Vec<char> = text.chars().collect();
    let folded = fold(&haystack);
    let needle = fold(&from.chars().collect::<Vec<char>>());
    if needle.is_empty() || needle.len() > haystack.len() {
        return text.to_string();
    }

    let mut out = String::new();
    let mut i = 0;
    while i < haystack.len() {
        let fits = i + needle.len() <= haystack.len()
            && folded[i..i + needle.len()] == needle[..]
            && boundary(&haystack, i, needle.len());
        if fits {
            out.push_str(&matching_case(haystack[i], to));
            i += needle.len();
        } else {
            out.push(haystack[i]);
            i += 1;
        }
    }
    out
}

/// Whether the text says this, as a word rather than as a fragment of one, ignoring case.
fn contains_word(text: &str, word: &str) -> bool {
    let haystack: Vec<char> = text.chars().collect();
    let folded = fold(&haystack);
    let needle = fold(&word.chars().collect::<Vec<char>>());
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    (0..=haystack.len() - needle.len())
        .any(|i| folded[i..i + needle.len()] == needle[..] && boundary(&haystack, i, needle.len()))
}

/// Whether a match at `start` stands alone rather than inside a longer word.
fn boundary(chars: &[char], start: usize, len: usize) -> bool {
    let before = start == 0 || !chars[start - 1].is_alphanumeric();
    let end = start + len;
    let after = end >= chars.len() || !chars[end].is_alphanumeric();
    before && after
}

/// Lower case, one character in one character out.
///
/// The offsets found in the folded text are used to index the original, so anything that changed
/// the character count would misalign every match after it. Vietnamese and ASCII both fold
/// one-to-one; anything that does not is left as it is rather than shifting the alignment.
fn fold(chars: &[char]) -> Vec<char> {
    chars
        .iter()
        .map(|c| {
            let mut lower = c.to_lowercase();
            match (lower.next(), lower.next()) {
                (Some(single), None) => single,
                _ => *c,
            }
        })
        .collect()
}

/// The replacement, capitalised if what it replaces was.
fn matching_case(first_replaced: char, replacement: &str) -> String {
    if !first_replaced.is_uppercase() {
        return replacement.to_string();
    }
    let mut chars = replacement.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Removes wording the target language's own interface register says does not belong.
///
/// Vietnamese interface text takes no pronoun at all - a button says "Thoát", not "Bạn thoát" -
/// and the register profiles already carry that as data. So this drops what they name rather than
/// deciding for itself what a label can spare.
fn from_dropped_words(current: &str, to: &Language, out: &mut Vec<Alternative>) {
    for profile in register::profiles_for(to) {
        // Only the interface register: dropping a pronoun out of dialogue changes who is speaking.
        if !profile.id.contains("ui") {
            continue;
        }
        for (word, _) in &profile.avoid {
            let trimmed = drop_word(current, word);
            if trimmed == current {
                continue;
            }
            out.push(Alternative {
                text: trimmed,
                width: None,
                why: format!(
                    "bỏ \"{word}\" - chữ trên nút không dùng đại từ ({})",
                    profile.id
                ),
            });
        }
    }
}

/// Removes a whole word, tidying the spaces it leaves behind.
fn drop_word(text: &str, word: &str) -> String {
    let kept: Vec<&str> = text
        .split_whitespace()
        .filter(|part| {
            let bare = part.trim_matches(|c: char| !c.is_alphanumeric());
            bare.to_lowercase() != word.to_lowercase()
        })
        .collect();
    kept.join(" ")
}
