//! Archive-level tests, including the hostile-input protections from specification §29.

use std::io::{Cursor, Write};
use tjlocalizer_core::jar::{Archive, ArchiveLimits, Manifest};

fn fixture_jar() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/sample-game.jar"
    ))
    .expect("fixture missing - run tools/make-fixtures.sh")
}

/// Builds an archive containing exactly the given entries, bypassing any path checks a normal
/// writer would apply - which is what a hostile archive does.
fn craft(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buffer);
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        for (name, data) in entries {
            zip.start_file(name.to_string(), options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    buffer.into_inner()
}

#[test]
fn reads_a_real_jar() {
    let archive = Archive::read(&fixture_jar()).unwrap();
    assert!(archive.get("SampleGame.class").is_some());
    assert!(archive.get("META-INF/MANIFEST.MF").is_some());
    assert!(archive.get("levels.properties").is_some());
    assert_eq!(archive.classes().count(), 1);
    assert_eq!(archive.sha256.len(), 64);
}

#[test]
fn rebuilding_is_deterministic() {
    // Two builds of the same content must hash identically, or "reproducible build" in the
    // specification means nothing and no output can be verified against a published hash.
    let archive = Archive::read(&fixture_jar()).unwrap();
    assert_eq!(archive.write().unwrap(), archive.write().unwrap());
}

#[test]
fn survives_a_write_read_cycle() {
    let mut archive = Archive::read(&fixture_jar()).unwrap();
    archive.insert("extra.txt", b"hello".to_vec());
    let rebuilt = Archive::read(&archive.write().unwrap()).unwrap();
    assert_eq!(rebuilt.get("extra.txt").unwrap().data, b"hello");
    assert!(rebuilt.get("SampleGame.class").is_some());
}

#[test]
fn refuses_path_traversal() {
    for hostile in ["../escaped.txt", "../../etc/passwd", "a/../../b.txt"] {
        let bytes = craft(&[(hostile, b"x".to_vec())]);
        let result = Archive::read(&bytes);
        assert!(result.is_err(), "{hostile:?} was accepted");
    }
}

#[test]
fn refuses_absolute_paths() {
    let bytes = craft(&[("/etc/passwd", b"x".to_vec())]);
    assert!(Archive::read(&bytes).is_err());
}

#[test]
fn refuses_an_oversized_entry() {
    let bytes = craft(&[("big.bin", vec![0u8; 4096])]);
    let limits = ArchiveLimits {
        max_entry_size: 1024,
        ..Default::default()
    };
    // A zip bomb compresses to almost nothing, so the limit has to bite on the uncompressed
    // size. This entry is 4 KiB of zeroes: tiny on disk, over the limit once expanded.
    assert!(Archive::read_with_limits(&bytes, &limits).is_err());
    assert!(
        Archive::read(&bytes).is_ok(),
        "the default limit should allow 4 KiB"
    );
}

#[test]
fn refuses_too_many_entries() {
    let entries: Vec<(String, Vec<u8>)> = (0..50)
        .map(|i| (format!("f{i}.txt"), b"x".to_vec()))
        .collect();
    let borrowed: Vec<(&str, Vec<u8>)> = entries
        .iter()
        .map(|(n, d)| (n.as_str(), d.clone()))
        .collect();
    let bytes = craft(&borrowed);
    let limits = ArchiveLimits {
        max_entries: 10,
        ..Default::default()
    };
    assert!(Archive::read_with_limits(&bytes, &limits).is_err());
}

#[test]
fn parses_a_midlet_manifest() {
    let archive = Archive::read(&fixture_jar()).unwrap();
    let text = String::from_utf8_lossy(&archive.get("META-INF/MANIFEST.MF").unwrap().data);
    let manifest = Manifest::parse(&text);

    assert_eq!(manifest.get("MIDlet-Name"), Some("Sample Game"));
    assert_eq!(manifest.get("MicroEdition-Configuration"), Some("CLDC-1.1"));
    assert_eq!(manifest.midlet_classes(), vec!["SampleGame".to_string()]);
}

#[test]
fn joins_continuation_lines() {
    // The format wraps at 72 bytes; treating each physical line as a record truncates the MIDlet
    // declaration and loses the entry-point class.
    let text = "Manifest-Version: 1.0\nMIDlet-1: A Game With A Very Long Name Indeed That W\n raps,/icon.png,com/example/Main\n";
    let manifest = Manifest::parse(text);
    assert_eq!(
        manifest.midlet_classes(),
        vec!["com/example/Main".to_string()]
    );
}

#[test]
fn manifest_render_round_trips() {
    let archive = Archive::read(&fixture_jar()).unwrap();
    let text = String::from_utf8_lossy(&archive.get("META-INF/MANIFEST.MF").unwrap().data);
    let manifest = Manifest::parse(&text);
    let reparsed = Manifest::parse(&manifest.render());

    assert_eq!(reparsed.get("MIDlet-Name"), manifest.get("MIDlet-Name"));
    assert_eq!(reparsed.midlet_classes(), manifest.midlet_classes());
}

#[test]
fn manifest_wrapping_never_splits_a_character() {
    let mut manifest = Manifest::default();
    manifest.set("Manifest-Version", "1.0");
    // Long non-ASCII value: naive byte-wise wrapping cuts a multi-byte character in half and the
    // device shows mojibake.
    manifest.set(
        "MIDlet-Name",
        "Trò chơi phiêu lưu kỳ ảo của Thanhtinz bản tiếng Việt hoàn chỉnh",
    );
    let rendered = manifest.render();
    assert!(rendered.is_char_boundary(rendered.len()));
    assert_eq!(
        Manifest::parse(&rendered).get("MIDlet-Name"),
        manifest.get("MIDlet-Name")
    );
}
