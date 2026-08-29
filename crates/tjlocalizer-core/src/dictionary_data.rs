//! The dictionary packs shipped with this build.
//!
//! Embedded rather than loaded from disk so the tool works out of the box and a project cannot be
//! half-usable because a data directory was not copied alongside the binary. A project's own
//! packs, in its `dictionary/` directory, are loaded on top and take precedence by ordering.
//!
//! These are game dictionaries, not general ones. Entries are chosen and tagged for the readings
//! a game needs: `装备` is `trang bị`, never `thiết bị`, which is hardware; `Guild` is `bang hội`,
//! never `hiệp hội`, which is a trade association. General-purpose dictionaries get these wrong
//! consistently, and that is exactly what makes a machine translation of a game read as one.

use crate::dictionary::{Dictionary, Pack};

/// The embedded packs, as JSON.
const PACKS: &[(&str, &str)] = &[
    ("zh-vi", include_str!("../data/dictionary/zh-vi.json")),
    ("en-vi", include_str!("../data/dictionary/en-vi.json")),
    ("ja-vi", include_str!("../data/dictionary/ja-vi.json")),
    ("ko-vi", include_str!("../data/dictionary/ko-vi.json")),
    ("ru-vi", include_str!("../data/dictionary/ru-vi.json")),
    ("en-zh", include_str!("../data/dictionary/en-zh.json")),
    ("en-th", include_str!("../data/dictionary/en-th.json")),
    ("en-id", include_str!("../data/dictionary/en-id.json")),
];

/// Every built-in pack.
///
/// A pack that fails to parse is skipped rather than panicking, and `builtin_errors` reports it.
/// These files are compiled in, so a failure means this build is broken, not the user's project -
/// but taking down a translator's whole session over it helps nobody.
pub fn builtin() -> Dictionary {
    let mut dictionary = Dictionary::default();
    for (_, json) in PACKS {
        if let Ok(pack) = serde_json::from_str::<Pack>(json) {
            dictionary.add(pack);
        }
    }
    dictionary
}

/// Packs that failed to parse, by name. Empty in a sound build; the test suite asserts that.
pub fn builtin_errors() -> Vec<(String, String)> {
    PACKS
        .iter()
        .filter_map(|(name, json)| match serde_json::from_str::<Pack>(json) {
            Ok(_) => None,
            Err(e) => Some((name.to_string(), e.to_string())),
        })
        .collect()
}
