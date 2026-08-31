//! What has been done to this project, in the order it happened.
//!
//! Localizing a game is not one sitting. Somebody imports a game on Sunday, extracts the text,
//! approves forty lines, and comes back three weeks later to a folder of JSON files that says
//! perfectly what the state *is* and nothing at all about how it got there or why they stopped.
//!
//! So the milestones are recorded as they happen, and a person can add a line of their own. Two
//! properties matter more than anything the format does:
//!
//! * **Append-only.** One JSON object per line, never rewritten. A log that gets rewritten is a log
//!   that can lose an entry, and the entry it loses is the one from the day something went wrong.
//! * **Facts, not inferences.** "build 3 failed validation with 2 errors" is worth reading in a
//!   month. "the project is 60% done" is a number that was true for an afternoon.
//!
//! It is deliberately not a database and deliberately not clever: `journal.jsonl` sits in the
//! project root, and `tail` and `git diff` both work on it.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

/// The file, in the project root beside `project.json`.
pub const FILE: &str = "journal.jsonl";

/// One thing that happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// When, as RFC 3339 in UTC. A string rather than a number because this file is read by
    /// people, and an epoch second is not something anybody reads.
    pub at: String,
    /// What kind of thing happened - `import`, `extract`, `build`, `rule`, `patch`, `note`. Kept
    /// short and stable so a later reader can filter without parsing prose.
    pub kind: String,
    /// The language it concerned, where it concerned one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub language: String,
    /// What happened, in a sentence a person wrote or the tool wrote for them.
    pub detail: String,
}

impl Entry {
    pub fn new(kind: &str, detail: impl Into<String>) -> Self {
        Entry {
            at: now(),
            kind: kind.to_string(),
            language: String::new(),
            detail: detail.into(),
        }
    }

    pub fn about(mut self, language: &crate::lang::Language) -> Self {
        self.language = language.tag().to_string();
        self
    }
}

/// Adds one entry.
///
/// Opened in append mode and written as a single line, so two processes writing at once interleave
/// whole entries rather than corrupting each other's - which is the failure a log that seeks and
/// rewrites would have.
///
/// A journal that cannot be written **does not fail the thing it was recording**: a build that
/// worked must not be reported as broken because a log line could not be appended. The error is
/// returned so a caller that cares can say so, and every caller here ignores it on purpose.
pub fn append(root: &Path, entry: &Entry) -> std::io::Result<()> {
    let line = match serde_json::to_string(entry) {
        Ok(line) => line,
        Err(e) => return Err(std::io::Error::other(e)),
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join(FILE))?;
    writeln!(file, "{line}")
}

/// Records an entry, and says nothing if it could not.
///
/// The shape every caller in this crate uses: the work is what matters, and the log is a note
/// about the work.
pub fn record(root: &Path, entry: Entry) {
    let _ = append(root, &entry);
}

/// Every entry, oldest first.
///
/// A line that will not parse is skipped rather than failing the read. The point of an append-only
/// log is that the old entries survive whatever went wrong later, and refusing to show any of them
/// because the last one was truncated by a power cut gets that exactly backwards.
pub fn read(root: &Path) -> Vec<Entry> {
    let Ok(text) = std::fs::read_to_string(root.join(FILE)) else {
        return Vec::new();
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// The last `count` entries, oldest first.
pub fn tail(root: &Path, count: usize) -> Vec<Entry> {
    let all = read(root);
    let from = all.len().saturating_sub(count);
    all[from..].to_vec()
}

/// The current time, RFC 3339 in UTC, without pulling in a date library.
///
/// The crate has no time dependency and this is the only place that needs one. Converting seconds
/// since the epoch into a date is arithmetic with one awkward part - which years are leap years -
/// and that is less to carry than a dependency.
fn now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    let (hour, minute, second) = (rest / 3600, (rest % 3600) / 60, rest % 60);

    let mut year = 1970;
    let mut days = days as i64;
    loop {
        let length = if leap(year) { 366 } else { 365 };
        if days < length {
            break;
        }
        days -= length;
        year += 1;
    }
    let lengths = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0;
    while month < 12 && days >= lengths[month] {
        days -= lengths[month];
        month += 1;
    }
    format!(
        "{year:04}-{:02}-{:02}T{hour:02}:{minute:02}:{second:02}Z",
        month + 1,
        days + 1
    )
}

fn leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
