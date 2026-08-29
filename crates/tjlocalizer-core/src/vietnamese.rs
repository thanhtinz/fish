//! Checks that only apply to Vietnamese (specification §11).
//!
//! Everything general moved to `quality`, `translation` and `register`. What is left is what
//! genuinely cannot be stated about any other language.

use crate::translation::Issue;

/// Vietnamese-specific problems in a finished translation.
pub fn check(target: &str) -> Vec<Issue> {
    let mut issues = Vec::new();
    issues.extend(missing_diacritics(target));
    issues.extend(unconverted_input(target));
    issues
}

/// Vietnamese written without its diacritics.
///
/// "Bat dau tro choi" is readable to a Vietnamese speaker and wrong in a shipped game. It passes
/// every other check - right length, right script, right spacing - so without this it ships. The
/// test is statistical rather than exact: a run of Vietnamese words none of which carry a mark,
/// where the language's own frequencies make that very unlikely.
fn missing_diacritics(target: &str) -> Vec<Issue> {
    let words: Vec<&str> = target
        .split_whitespace()
        .filter(|w| w.chars().all(|c| c.is_alphabetic()))
        .collect();

    // Under four words there is nothing to be statistical about: "Quit", "OK" and "Menu" are all
    // legitimately mark-free, and so are proper nouns.
    if words.len() < 4 {
        return Vec::new();
    }
    if words.iter().any(|w| w.chars().any(is_vietnamese_mark)) {
        return Vec::new();
    }
    // Words that exist in Vietnamese without any diacritic at all - "cho", "ban", "khong" is not
    // one of them but "cho" and "ban" are - so only flag when the text also looks Vietnamese in
    // shape rather than simply being English.
    if !looks_vietnamese_without_marks(&words) {
        return Vec::new();
    }

    vec![Issue {
        code: "diacritics".into(),
        detail: format!(
            "{} words and not one diacritic - this looks like Vietnamese typed without marks",
            words.len()
        ),
    }]
}

/// Telex or VNI sequences left unconverted: "ddaay", "tie6ng", "ba5n".
///
/// A translator typing into a field with the wrong input method produces these, and they are
/// invisible to a spell check that does not know the input schemes.
fn unconverted_input(target: &str) -> Vec<Issue> {
    let mut found = Vec::new();
    for word in target.split_whitespace() {
        let lower = word.to_lowercase();
        // VNI puts a digit inside a word: "ba5n". A digit at either end is a number or an index.
        let inner_digit = lower
            .char_indices()
            .any(|(i, c)| c.is_ascii_digit() && i > 0 && i + c.len_utf8() < lower.len());
        let telex = lower.contains("ddd") || lower.contains("aaa") || lower.contains("eee");
        if (inner_digit && lower.chars().any(|c| c.is_alphabetic())) || telex {
            found.push(word.to_string());
        }
    }
    if found.is_empty() {
        return Vec::new();
    }
    vec![Issue {
        code: "input".into(),
        detail: format!(
            "looks like unconverted Telex or VNI input: {}",
            found.join(", ")
        ),
    }]
}

/// Whether a character carries a Vietnamese tone mark or vowel modification.
fn is_vietnamese_mark(c: char) -> bool {
    matches!(c as u32,
        0x00C0..=0x1EF9 if !c.is_ascii_alphabetic())
        || matches!(c, 'Đ' | 'đ')
}

/// Whether mark-free words look like Vietnamese rather than English.
///
/// Vietnamese syllables are short and its words are single syllables written apart, so a run of
/// short tokens with Vietnamese-shaped syllables is the signal. Checking against a list of common
/// mark-free Vietnamese syllables is more reliable than syllable shape alone, which English also
/// satisfies.
fn looks_vietnamese_without_marks(words: &[&str]) -> bool {
    const COMMON: &[&str] = &[
        "cho", "ban", "khong", "duoc", "nguoi", "choi", "trong", "tren", "vao", "ra", "va", "la",
        "cua", "den", "tu", "voi", "mot", "hai", "ba", "nhan", "vat", "diem", "kinh", "nghiem",
        "cap", "do", "tro", "bat", "dau", "ket", "thuc", "tiep", "tuc", "thoat", "luu", "tai",
        "xoa", "them", "sua", "tim", "kiem", "chon", "dong", "mo", "gui", "nhap", "xuat",
    ];
    let hits = words
        .iter()
        .filter(|w| COMMON.contains(&w.to_lowercase().as_str()))
        .count();
    // Two known syllables in a run of four or more is well past chance for English text.
    hits >= 2
}
