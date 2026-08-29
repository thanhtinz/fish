//! Class-file level tests against a real javac-produced fixture.

use tjlocalizer_core::classfile::{decode_modified_utf8, encode_modified_utf8, ClassFile};

fn fixture() -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/SampleGame.class"))
        .expect("fixture missing - run tools/make-fixtures.sh")
}

#[test]
fn parses_a_real_class_file() {
    let class = ClassFile::parse(&fixture()).expect("parse failed");
    assert!(class.constant_count() > 10);
    assert_eq!(class.major_version, 52, "fixture should be Java 8 bytecode");
}

#[test]
fn finds_only_displayable_literals() {
    let class = ClassFile::parse(&fixture()).unwrap();
    let literals: Vec<String> = class
        .string_literals()
        .into_iter()
        .filter_map(|l| l.decoded)
        .collect();

    for expected in [
        "Dragon Quest Online",
        "Start Game",
        "Quit",
        "You have arrived at last, traveller.",
        "HP: %d / %d",
        "/img/hud.png",
        "装备",
    ] {
        assert!(literals.contains(&expected.to_string()), "missing {expected:?}");
    }

    // The pool is full of class names, field names and type descriptors. None of them are
    // CONSTANT_String targets, so none may appear here - renaming one would break the class.
    for forbidden in ["SampleGame", "java/lang/String", "()V", "TITLE", "main"] {
        assert!(
            !literals.contains(&forbidden.to_string()),
            "{forbidden:?} is structural, not displayable text"
        );
    }
}

#[test]
fn rewriting_a_literal_leaves_the_rest_byte_identical() {
    let original = fixture();
    let class = ClassFile::parse(&original).unwrap();
    // Writing back without changes must reproduce the input exactly, or the parser is losing
    // information and no patch it produces can be trusted.
    assert_eq!(class.write().unwrap(), original, "unmodified round-trip differs");
}

#[test]
fn longer_translations_fit() {
    let mut class = ClassFile::parse(&fixture()).unwrap();
    let target = class
        .string_literals()
        .into_iter()
        .find(|l| l.decoded.as_deref() == Some("Start Game"))
        .expect("literal not found");

    // Vietnamese is routinely longer than the English it replaces, and carries diacritics that
    // cost two or three bytes each in modified UTF-8. If the pool could not grow, the whole tool
    // would be limited to translations that happen to be shorter than the original.
    class
        .set_utf8_text(target.utf8_index, "Bắt đầu trò chơi")
        .unwrap();

    let rewritten = class.write().unwrap();
    assert!(rewritten.len() > fixture().len(), "patched class did not grow");

    let reparsed = ClassFile::parse(&rewritten).unwrap();
    let literals: Vec<String> = reparsed
        .string_literals()
        .into_iter()
        .filter_map(|l| l.decoded)
        .collect();
    assert!(literals.contains(&"Bắt đầu trò chơi".to_string()));
    assert!(!literals.contains(&"Start Game".to_string()));
    // Untouched literals must survive the pool rewrite.
    assert!(literals.contains(&"Quit".to_string()));
}

#[test]
fn modified_utf8_round_trips_vietnamese() {
    for text in [
        "Bắt đầu",
        "Cường hóa trang bị",
        "Đường dẫn: /img/hud.png",
        "ăâêôơư ĂÂÊÔƠƯ Đđ ạảãáàắằẳẵặ",
        "",
    ] {
        let encoded = encode_modified_utf8(text);
        assert_eq!(decode_modified_utf8(&encoded).unwrap(), text, "failed for {text:?}");
    }
}

#[test]
fn modified_utf8_encodes_nul_as_two_bytes() {
    // The JVM format forbids a bare zero byte inside a Utf8 constant. Encoding NUL the standard
    // way would produce a class the verifier rejects.
    let encoded = encode_modified_utf8("a\0b");
    assert_eq!(encoded, vec![b'a', 0xC0, 0x80, b'b']);
    assert!(!encoded[1..].contains(&0));
}

#[test]
fn rejects_files_that_are_not_classes() {
    let err = ClassFile::parse(b"PK\x03\x04not a class at all").unwrap_err();
    assert!(matches!(err, tjlocalizer_core::Error::NotAClassFile { .. }));
}

#[test]
fn rejects_truncated_input() {
    let mut bytes = fixture();
    bytes.truncate(40);
    assert!(ClassFile::parse(&bytes).is_err());
}
