//! What may be written back, and what must be left alone.
//!
//! The strongest assertion in this file is a negative one, and negatives are the thing
//! self-generated fixtures prove completely: *these bytes were not touched*. That needs no real
//! game to be certain of.
//!
//! It matters because of what the build used to do. Every patched resource was decoded with
//! `from_utf8_lossy` and written back. Nothing had a node in a binary file yet, so nothing broke -
//! but the first reader for Android bytecode would have turned every invalid byte of a
//! `classes.dex` into U+FFFD, written the result back, and reported success.

use tjlocalizer_core::jar::Archive;
use tjlocalizer_core::project::Project;
use tjlocalizer_core::resource::Format;
use tjlocalizer_core::writeback::{self, BinaryFormat, Plan};

fn fixture() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/sample-game.jar"
    ))
    .expect("fixture missing - run tools/make-fixtures.sh")
}

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tjlocalizer-writeback-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Bytes that are not text and match no reader are refused, not guessed at.
#[test]
fn anything_unrecognised_is_read_only_by_default() {
    let bytes: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
    match writeback::plan("mystery.bin", &bytes) {
        Plan::ReadOnly { reason } => assert!(!reason.is_empty()),
        other => panic!("unrecognised bytes were not refused: {other:?}"),
    }
}

/// Each binary format this build knows of but cannot write says which it is, rather than being
/// lumped together as "binary" - the three states a person has to tell apart are "text you can
/// change", "text you cannot change yet", and "not text".
#[test]
fn a_known_but_unwritable_format_says_which_one_it_is() {
    for (name, expected) in [
        ("classes.dex", "Android bytecode"),
        ("resources.arsc", "resource table"),
        ("data.unity3d", "Unity"),
        ("game.pck", "Godot"),
        ("archive.rpa", "Ren'Py"),
    ] {
        match writeback::plan(name, b"\x00\x01\x02binary") {
            Plan::ReadOnly { reason } => assert!(
                reason.contains(expected),
                "{name} said {reason:?}, which does not mention {expected}"
            ),
            other => panic!("{name} was not refused: {other:?}"),
        }
    }
}

#[test]
fn text_is_planned_with_its_format_and_its_character_set() {
    match writeback::plan("levels.properties", b"a=1\nb=2\n") {
        Plan::Text { format, encoding } => {
            assert_eq!(format, Format::Properties);
            assert!(!encoding.is_empty());
        }
        other => panic!("a properties file was not planned as text: {other:?}"),
    }
}

/// A `.dex` that happens to decode as text must still be refused: what decides is what this build
/// can write, not whether the bytes look friendly.
#[test]
fn a_binary_format_that_decodes_as_text_is_still_refused() {
    let plan = writeback::plan("classes.dex", b"this file is all printable ascii\n");
    assert!(!plan.writable(), "{plan:?}");
}

/// The one that matters. A translation approved against a file this build cannot write must leave
/// that file byte-identical - and say so rather than quietly succeeding.
#[test]
fn a_refused_resource_comes_out_byte_identical_and_is_reported() {
    use tjlocalizer_core::graph::{ContentGraph, TextSource};
    use tjlocalizer_core::translation::TranslationStore;

    // Bytecode-shaped: mostly unprintable, with a readable string inside, which is exactly the
    // shape that a lossy decode mangles most convincingly.
    let mut dex = b"dex\n035\0".to_vec();
    dex.extend_from_slice(&[0x00, 0x9C, 0xFF, 0xFE, 0x01]);
    dex.extend_from_slice(b"Start Game");
    dex.extend_from_slice(&[0x00, 0xC3, 0x28, 0xFF]);

    let mut archive = Archive::read(&fixture()).unwrap();
    archive.insert("classes.dex", dex.clone());

    let dir = TempDir::new("refused");
    let project = Project::create(&dir.0, "game", &archive.write().unwrap()).unwrap();
    project.extract().unwrap();

    // Nothing extracts nodes from a .dex yet, so the translation is aimed at it by hand - which is
    // precisely the state the first DEX reader will create.
    let language = project.active_targets()[0].language.clone();
    let mut graph: ContentGraph = project.graph().unwrap();
    let node = tjlocalizer_core::graph::TextNode {
        id: "handmade".into(),
        source: TextSource::ResourceProperty {
            resource: "classes.dex".into(),
            key: "#1".into(),
        },
        source_text: "Start Game".into(),
        source_encoding: None,
        context: tjlocalizer_core::graph::ContextType::Ui,
        constraints: Default::default(),
    };
    graph.nodes.push(node);
    std::fs::write(
        dir.0.join("content/graph.json"),
        serde_json::to_string(&graph).unwrap(),
    )
    .unwrap();

    let mut translations = TranslationStore::default();
    translations.set("handmade", "Bắt đầu");
    project.save_translations(&language, &translations).unwrap();

    let record = project.build(&language).unwrap();

    // The bytes, first and above all.
    let output = project.output_path(&language).unwrap().unwrap();
    let built = Archive::read(&std::fs::read(output).unwrap()).unwrap();
    assert_eq!(
        built.get("classes.dex").unwrap().data,
        dex,
        "the build modified a file it cannot write"
    );

    // And it said so, with the count, rather than reporting a clean build.
    let refusal = record
        .report
        .refused
        .iter()
        .find(|r| r.resource == "classes.dex")
        .expect("nothing recorded that the file was left alone");
    assert_eq!(refusal.translations, 1);

    let finding = record
        .validation
        .findings
        .iter()
        .find(|f| f.check == "text.unwritable")
        .expect("the translation vanished without a warning");
    assert!(finding.detail.contains("classes.dex"), "{}", finding.detail);
    assert!(
        finding.detail.contains("1 approved translation "),
        "the count should be singular here: {}",
        finding.detail
    );
}

/// `analyze` and `extract` have to agree, because they now ask the same question. They did not
/// always: one recognised Unreal's table by file extension and swallowed parse failures, the other
/// by magic bytes and reported them.
#[test]
fn what_the_survey_calls_readable_is_what_extraction_reads() {
    let dir = TempDir::new("agree");
    let mut archive = Archive::read(&fixture()).unwrap();
    archive.insert("junk.bin", (0u8..=255).cycle().take(2048).collect());
    let project = Project::create(&dir.0, "game", &archive.write().unwrap()).unwrap();

    let readable: Vec<String> = project
        .package()
        .unwrap()
        .readable
        .into_iter()
        .map(|r| r.entry)
        .collect();
    let graph = project.extract().unwrap();

    for entry in &readable {
        let found = graph
            .nodes
            .iter()
            .any(|n| format!("{:?}", n.source).contains(entry.as_str()));
        assert!(found, "{entry} was called readable but produced no nodes");
    }
    assert!(
        !readable.iter().any(|e| e == "junk.bin"),
        "random bytes were called readable"
    );
}

#[test]
fn locres_is_still_read_and_written_through_the_same_decision() {
    // A minimal valid table, built the way the format documents.
    let magic: [u8; 16] = [
        0x0E, 0x14, 0x74, 0x75, 0x67, 0x4A, 0x03, 0xFC, 0x4A, 0x15, 0x90, 0x9D, 0xC3, 0x37, 0x7F,
        0x1B,
    ];
    let mut body = Vec::new();
    body.extend_from_slice(&1u32.to_le_bytes()); // entries (v3)
    body.extend_from_slice(&1u32.to_le_bytes()); // namespaces
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&5i32.to_le_bytes());
    body.extend_from_slice(b"Game\0");
    body.extend_from_slice(&1u32.to_le_bytes()); // keys
    body.extend_from_slice(&0u32.to_le_bytes());
    body.extend_from_slice(&4i32.to_le_bytes());
    body.extend_from_slice(b"KEY\0");
    body.extend_from_slice(&7u32.to_le_bytes()); // source hash
    body.extend_from_slice(&0i32.to_le_bytes()); // string index

    let mut bytes = magic.to_vec();
    bytes.push(3);
    let at = (bytes.len() + 8 + body.len()) as i64;
    bytes.extend_from_slice(&at.to_le_bytes());
    bytes.extend_from_slice(&body);
    bytes.extend_from_slice(&1i32.to_le_bytes());
    bytes.extend_from_slice(&6i32.to_le_bytes());
    bytes.extend_from_slice(b"Hello\0");
    bytes.extend_from_slice(&1i32.to_le_bytes());

    assert_eq!(
        writeback::plan("Game.locres", &bytes),
        Plan::Binary(BinaryFormat::Locres)
    );
    // And a renamed one is still one: the bytes decide, not the name.
    assert_eq!(
        writeback::plan("strings.dat", &bytes),
        Plan::Binary(BinaryFormat::Locres)
    );
}
