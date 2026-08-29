//! Whether a translation is publishable, in the target language's own terms (specification §24).
//!
//! Every rule here is parameterised by language, because none of them is universal. Spacing
//! before a comma is wrong in Vietnamese and meaningless in Chinese, which uses full-width
//! punctuation that carries its own spacing. A length budget tuned for Vietnamese would flag
//! every correct Chinese translation, since Chinese says the same thing in far fewer characters.
//! Getting this wrong does not merely produce noise: a check that fires on everything is one a
//! translator learns to ignore, which is worse than not having it.

use crate::lang::{Language, LanguageProfile, Script};
use crate::translation::{Glossary, Issue};

/// Tidies text without changing its wording.
///
/// Only mechanical fixes: collapsed whitespace, and spacing around the punctuation the language
/// actually spaces. Anything that would alter meaning or tone belongs to a translator.
pub fn normalize(text: &str, language: &Language) -> String {
    let profile = language.profile();
    let mut out = String::with_capacity(text.len());
    let mut last_was_space = false;

    for c in text.trim().chars() {
        if c.is_whitespace() {
            // A script that does not space its words still uses spaces meaningfully when they
            // appear - between a number and a unit, say - so they are collapsed, not dropped.
            if !last_was_space {
                out.push(' ');
            }
            last_was_space = true;
            continue;
        }
        if profile.tight_punctuation.contains(&c) {
            while out.ends_with(' ') {
                out.pop();
            }
        }
        out.push(c);
        last_was_space = false;
    }
    out
}

/// Checks a translation against its source before it can be approved.
pub fn check(
    source: &str,
    target: &str,
    placeholders: &[String],
    from: &Language,
    to: &Language,
) -> Vec<Issue> {
    let mut issues = Vec::new();

    if target.trim().is_empty() {
        issues.push(Issue {
            code: "empty".into(),
            detail: "translation is empty".into(),
        });
        return issues;
    }

    // Every placeholder in the source must appear the same number of times in the target. A lost
    // "%d" is not cosmetic: the runtime format call will be wrong or will throw.
    for placeholder in placeholders {
        let wanted = source.matches(placeholder.as_str()).count();
        let got = target.matches(placeholder.as_str()).count();
        if wanted != got {
            issues.push(Issue {
                code: "placeholder".into(),
                detail: format!("{placeholder} appears {got} times, expected {wanted}"),
            });
        }
    }

    if target != normalize(target, to) {
        issues.push(Issue {
            code: "spacing".into(),
            detail: "leading, trailing or duplicated whitespace".into(),
        });
    }

    // A translation several times longer than its source usually means an explanation was written
    // into a label with no room for it. The budget accounts for how densely each script writes.
    let profile: LanguageProfile = to.profile();
    let source_len = source.chars().count();
    let target_len = target.chars().count();
    let budget = profile.length_budget(source_len, from);
    if source_len > 0 && target_len > budget && target_len > 24 {
        issues.push(Issue {
            code: "length".into(),
            detail: format!(
                "{target_len} characters against a {source_len} character source (budget {budget})"
            ),
        });
    }

    issues.extend(script_issues(target, to));
    issues
}

/// Problems visible from the target's writing system alone.
fn script_issues(target: &str, to: &Language) -> Vec<Issue> {
    let mut issues = Vec::new();
    let script = to.script();

    // Text left in the source script is text that was never translated. This catches the common
    // half-finished case - a Chinese term carried into a Vietnamese line - which no other check
    // sees, because such a line is well spaced, the right length and has all its placeholders.
    let stray = target
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_ascii() && script_of(*c) != Some(script))
        .count();
    if stray > 0 && script != Script::Han && script != Script::Japanese {
        issues.push(Issue {
            code: "script".into(),
            detail: format!("{stray} characters are not written in the target's script"),
        });
    }

    issues
}

/// The script a character belongs to, for the ranges this tool meets. `None` for anything else,
/// which is treated as acceptable rather than guessed at.
fn script_of(c: char) -> Option<Script> {
    let c = c as u32;
    Some(match c {
        0x0041..=0x024F | 0x1E00..=0x1EFF => Script::Latin,
        0x0400..=0x04FF => Script::Cyrillic,
        0x0E00..=0x0E7F => Script::Thai,
        0x3040..=0x30FF => Script::Japanese,
        0xAC00..=0xD7AF | 0x1100..=0x11FF => Script::Korean,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF => Script::Han,
        0x0600..=0x06FF => Script::Arabic,
        _ => return None,
    })
}

/// Applies locked glossary terms to a translation, reporting the ones that disagree.
///
/// Returns issues rather than rewriting: a locked term appearing with the wrong wording usually
/// means the whole sentence was built around the wrong reading, and substituting it silently
/// would produce something ungrammatical.
pub fn check_glossary(target: &str, source: &str, glossary: &Glossary) -> Vec<Issue> {
    let mut issues = Vec::new();
    for entry in glossary.matches_in(source) {
        if entry.locked && !target.contains(&entry.target) {
            issues.push(Issue {
                code: "glossary".into(),
                detail: format!(
                    "source contains the locked term {:?}, which must be translated as {:?}",
                    entry.source, entry.target
                ),
            });
        }
    }
    issues
}
