//! Unreal Engine's compiled string table.
//!
//! A caveat these tests cannot remove, and which the documentation states plainly: the files here
//! are written by this build. They prove the reader and the writer agree with each other and that
//! everything not translated survives; they cannot prove the format matches what a shipped game
//! expects, because there is no shipped game here to check against. That is why the parser
//! refuses anything it does not recognise instead of reading it hopefully.

use tjlocalizer_core::locres::Locres;

const MAGIC: [u8; 16] = [
    0x0E, 0x14, 0x74, 0x75, 0x67, 0x4A, 0x03, 0xFC, 0x4A, 0x15, 0x90, 0x9D, 0xC3, 0x37, 0x7F, 0x1B,
];

/// Builds a table by hand, so what the parser is being asked to read is visible here rather than
/// produced by the code under test.
fn table(version: u8, entries: &[(&str, &str, u32, i32)], strings: &[(&str, i32)]) -> Vec<u8> {
    fn string(out: &mut Vec<u8>, text: &str) {
        if text.is_ascii() {
            out.extend_from_slice(&((text.len() + 1) as i32).to_le_bytes());
            out.extend_from_slice(text.as_bytes());
            out.push(0);
        } else {
            let units: Vec<u16> = text.encode_utf16().collect();
            out.extend_from_slice(&(-((units.len() + 1) as i32)).to_le_bytes());
            for unit in units {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            out.extend_from_slice(&[0, 0]);
        }
    }

    // One namespace holding every entry, which is how a small game's table looks.
    let mut body = Vec::new();
    if version >= 3 {
        body.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    }
    body.extend_from_slice(&1u32.to_le_bytes()); // namespaces
    body.extend_from_slice(&0xAAAA_BBBBu32.to_le_bytes());
    string(&mut body, "Game");
    body.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (key, _, source_hash, index) in entries {
        body.extend_from_slice(&0x1111_2222u32.to_le_bytes());
        string(&mut body, key);
        body.extend_from_slice(&source_hash.to_le_bytes());
        body.extend_from_slice(&index.to_le_bytes());
    }

    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    out.push(version);
    let strings_at = (out.len() + 8 + body.len()) as i64;
    out.extend_from_slice(&strings_at.to_le_bytes());
    out.extend_from_slice(&body);

    out.extend_from_slice(&(strings.len() as i32).to_le_bytes());
    for (text, refs) in strings {
        string(&mut out, text);
        out.extend_from_slice(&refs.to_le_bytes());
    }
    out
}

fn sample() -> Vec<u8> {
    table(
        3,
        &[
            ("MENU_START", "", 0xDEAD_BEEF, 0),
            ("MENU_QUIT", "", 0x0BAD_F00D, 1),
        ],
        &[("Start Game", 1), ("Quit", 1)],
    )
}

#[test]
fn a_table_is_read_with_its_namespaces_keys_and_hashes() {
    let locres = Locres::parse(&sample()).unwrap();
    let entries = locres.entries();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].namespace, "Game");
    assert_eq!(entries[0].key, "MENU_START");
    assert_eq!(entries[0].text, "Start Game");
    assert_eq!(entries[0].source_hash, 0xDEAD_BEEF);
    assert_eq!(entries[1].text, "Quit");
}

/// The point of writing it back: everything not translated has to come out the way it went in.
#[test]
fn writing_it_back_unchanged_produces_the_same_entries() {
    let locres = Locres::parse(&sample()).unwrap();
    let again = Locres::parse(&locres.write()).unwrap();
    assert_eq!(locres.entries(), again.entries());
}

#[test]
fn a_translated_entry_survives_a_round_trip_and_leaves_the_others_alone() {
    let mut locres = Locres::parse(&sample()).unwrap();
    assert!(locres.set("Game", "MENU_START", "Bắt đầu"));

    let again = Locres::parse(&locres.write()).unwrap();
    let entries = again.entries();
    assert_eq!(entries[0].text, "Bắt đầu");
    assert_eq!(entries[1].text, "Quit", "an untouched entry changed");

    // The source hash is Unreal's way of noticing a translation has gone stale. Inventing a new
    // one would tell the engine every string had been re-checked against a source nobody read.
    assert_eq!(entries[0].source_hash, 0xDEAD_BEEF);
}

/// Vietnamese does not fit in one byte per character, and a table written as if it did would hand
/// the engine bytes that are not the text.
#[test]
fn a_non_ascii_translation_is_written_as_wide_characters() {
    let mut locres = Locres::parse(&sample()).unwrap();
    locres.set("Game", "MENU_QUIT", "Thoát trò chơi");

    let bytes = locres.write();
    let again = Locres::parse(&bytes).unwrap();
    assert_eq!(again.entries()[1].text, "Thoát trò chơi");
}

/// Unreal stores each distinct text once and counts the references, so translating one entry
/// must not silently translate another that happened to say the same thing.
#[test]
fn two_entries_sharing_a_string_stay_independent() {
    let shared = table(3, &[("A", "", 1, 0), ("B", "", 2, 0)], &[("Close", 2)]);
    let mut locres = Locres::parse(&shared).unwrap();
    assert_eq!(locres.entries()[0].text, "Close");
    assert_eq!(locres.entries()[1].text, "Close");

    locres.set("Game", "A", "Đóng");
    let again = Locres::parse(&locres.write()).unwrap();
    assert_eq!(again.entries()[0].text, "Đóng");
    assert_eq!(
        again.entries()[1].text,
        "Close",
        "translating one entry changed another that shared its text"
    );
}

/// A binary format read slightly wrong produces text that looks almost right and a file that
/// crashes a game, and the second is discovered long after the first. So anything unrecognised is
/// refused, with a reason.
#[test]
fn anything_it_does_not_recognise_is_refused_with_a_reason() {
    let mut wrong_magic = sample();
    wrong_magic[0] = 0xFF;
    assert!(!Locres::looks_like(&wrong_magic));
    let message = Locres::parse(&wrong_magic).unwrap_err().to_string();
    assert!(message.contains("not a .locres"), "{message}");

    // Version 1 keeps its strings inline rather than in a shared array: a different layout, and
    // reading it with this parser would produce nonsense rather than an error.
    let mut old_version = sample();
    old_version[16] = 1;
    let message = Locres::parse(&old_version).unwrap_err().to_string();
    assert!(message.contains("version 1"), "{message}");

    // And a file that stops in the middle says so rather than panicking.
    let truncated = &sample()[..30];
    assert!(Locres::parse(truncated).is_err());
}

#[test]
fn version_two_is_read_as_well() {
    let bytes = table(2, &[("K", "", 7, 0)], &[("Hello", 1)]);
    let locres = Locres::parse(&bytes).unwrap();
    assert_eq!(locres.entries()[0].text, "Hello");

    // And written back as version 2, not silently upgraded.
    let written = locres.write();
    assert_eq!(written[16], 2);
    assert_eq!(Locres::parse(&written).unwrap().entries(), locres.entries());
}

#[test]
fn setting_an_entry_that_is_not_there_says_so() {
    let mut locres = Locres::parse(&sample()).unwrap();
    assert!(!locres.set("Game", "NOT_A_KEY", "x"));
    assert!(!locres.set("OtherNamespace", "MENU_START", "x"));
}

mod through_the_pipeline {
    use super::*;
    use tjlocalizer_core::jar::Archive;
    use tjlocalizer_core::project::Project;

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("tjlocalizer-locres-{tag}-{unique}"));
            std::fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A zip holding an Unreal string table, which is what a Steam game's content folder looks
    /// like once it is packed.
    fn game() -> Vec<u8> {
        let base = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/sample-game.jar"
        ))
        .expect("fixture missing - run tools/make-fixtures.sh");
        let mut archive = Archive::read(&base).unwrap();
        archive.remove("SampleGame.class");
        archive.remove("META-INF/MANIFEST.MF");
        archive.insert("Content/Localization/Game/en/Game.locres", sample());
        archive.write().unwrap()
    }

    #[test]
    fn an_unreal_table_goes_through_extract_translate_and_build() {
        let dir = TempDir::new("pipeline");
        let project = Project::create(&dir.0, "unreal", &game()).unwrap();

        // Recognised as text this build can read, not as something merely named.
        let package = project.package().unwrap();
        let found = package
            .readable
            .iter()
            .find(|r| r.entry.ends_with("Game.locres"))
            .expect("the table was not read");
        assert_eq!(found.format, "unreal-locres");
        assert_eq!(found.fields, 2);

        let graph = project.extract().unwrap();
        let start = graph
            .nodes
            .iter()
            .find(|n| n.source_text == "Start Game")
            .expect("the entry was not extracted");

        let language = project.active_targets()[0].language.clone();
        let mut translations = project.translations(&language).unwrap();
        translations.set(start.id.clone(), "Bắt đầu");
        project.save_translations(&language, &translations).unwrap();

        let record = project.build(&language).unwrap();
        assert_eq!(record.report.resources_patched, 1);

        // Read back out of the built file, which is the only thing that proves it.
        let output = project.output_path(&language).unwrap().unwrap();
        let built = Archive::read(&std::fs::read(output).unwrap()).unwrap();
        let table = Locres::parse(
            &built
                .get("Content/Localization/Game/en/Game.locres")
                .unwrap()
                .data,
        )
        .unwrap();

        let entries = table.entries();
        assert_eq!(entries[0].text, "Bắt đầu");
        assert_eq!(entries[1].text, "Quit", "an untranslated entry changed");
        assert_eq!(entries[0].source_hash, 0xDEAD_BEEF, "the source hash moved");
    }
}
