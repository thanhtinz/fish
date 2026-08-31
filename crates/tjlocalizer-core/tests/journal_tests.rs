//! What was done to a project, in the order it happened.
//!
//! The reason this exists is a person coming back three weeks later to a folder of JSON that says
//! perfectly what the state *is* and nothing about how it got there. So what is asserted here is
//! mostly that the record survives: that it is appended to rather than rewritten, that a corrupt
//! line does not take the good ones with it, and that a log that cannot be written never fails the
//! work it was recording.

use tjlocalizer_core::journal::{self, Entry};
use tjlocalizer_core::lang::Language;
use tjlocalizer_core::project::Project;

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tjlocalizer-journal-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/sample-game.jar"
    ))
    .expect("fixture missing - run tools/make-fixtures.sh")
}

/// The milestones that are worth a line in a month's time write themselves. Nobody remembers to
/// log the build they were in the middle of.
#[test]
fn the_steps_of_the_work_record_themselves() {
    let dir = TempDir::new("steps");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    project.extract().unwrap();
    project.build(&Language::new("vi-VN")).unwrap();

    let kinds: Vec<String> = project.journal().into_iter().map(|e| e.kind).collect();
    for expected in ["import", "extract", "build"] {
        assert!(
            kinds.iter().any(|k| k == expected),
            "no {expected}: {kinds:?}"
        );
    }
}

/// A build entry has to say whether it passed, not merely that it happened. "Build 3" is not what
/// somebody came back to find out.
#[test]
fn a_build_entry_says_whether_it_passed() {
    let dir = TempDir::new("verdict");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    project.extract().unwrap();
    let language = Language::new("vi-VN");
    project.build(&language).unwrap();

    let build = project
        .journal()
        .into_iter()
        .find(|e| e.kind == "build")
        .expect("no build entry");
    assert_eq!(build.language, "vi-VN");
    assert!(
        build.detail.contains("validation passed") || build.detail.contains("error"),
        "{}",
        build.detail
    );
}

/// The one thing no recorded milestone can know: why somebody stopped.
#[test]
fn a_person_can_write_down_where_they_left_off() {
    let dir = TempDir::new("note");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    project
        .note("waiting on a screenshot of the shop menu")
        .unwrap();

    let note = project.journal().into_iter().last().unwrap();
    assert_eq!(note.kind, "note");
    assert_eq!(note.detail, "waiting on a screenshot of the shop menu");
}

/// Append-only, and it matters: a log that is read, edited and rewritten is a log that can lose the
/// entry from the day something went wrong.
#[test]
fn entries_are_appended_and_never_rewritten() {
    let dir = TempDir::new("append");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    project.note("first").unwrap();
    let after_one = std::fs::read_to_string(dir.0.join(journal::FILE)).unwrap();

    project.note("second").unwrap();
    let after_two = std::fs::read_to_string(dir.0.join(journal::FILE)).unwrap();

    assert!(
        after_two.starts_with(&after_one),
        "the first entry was rewritten:\n{after_two}"
    );
    assert_eq!(after_two.lines().count(), 3, "{after_two}");
}

/// A truncated last line - a power cut mid-write - must not hide every entry before it. The whole
/// point of an append-only log is that the old entries survive whatever happened later.
#[test]
fn a_corrupt_line_does_not_take_the_good_ones_with_it() {
    let dir = TempDir::new("corrupt");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    project.note("this one is fine").unwrap();

    let mut text = std::fs::read_to_string(dir.0.join(journal::FILE)).unwrap();
    text.push_str("{\"at\":\"2026-01-01T00:00:00Z\",\"kin");
    std::fs::write(dir.0.join(journal::FILE), text).unwrap();

    let entries = project.journal();
    assert!(
        entries.iter().any(|e| e.detail == "this one is fine"),
        "{entries:?}"
    );
}

/// A log line that cannot be written must not fail the work it was recording. A build that worked
/// is not a broken build because a note about it could not be appended.
#[test]
fn a_journal_that_cannot_be_written_does_not_fail_the_build() {
    let dir = TempDir::new("readonly");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    project.extract().unwrap();

    // A directory where the file should be: nothing can append to it, on any platform.
    let path = dir.0.join(journal::FILE);
    let _ = std::fs::remove_file(&path);
    std::fs::create_dir_all(&path).unwrap();

    let record = project.build(&Language::new("vi-VN"));
    assert!(
        record.is_ok(),
        "the build failed over a log line: {record:?}"
    );
}

/// The timestamp is computed here rather than by a date library, so it is worth checking that it
/// is a date at all and one from this century.
#[test]
fn the_timestamp_is_a_readable_utc_date() {
    let entry = Entry::new("note", "x");
    assert_eq!(entry.at.len(), 20, "{}", entry.at);
    assert!(entry.at.ends_with('Z'), "{}", entry.at);

    let year: u32 = entry.at[..4].parse().expect("no year");
    assert!((2020..2100).contains(&year), "{}", entry.at);

    let month: u32 = entry.at[5..7].parse().expect("no month");
    let day: u32 = entry.at[8..10].parse().expect("no day");
    assert!((1..=12).contains(&month), "{}", entry.at);
    assert!((1..=31).contains(&day), "{}", entry.at);
}
