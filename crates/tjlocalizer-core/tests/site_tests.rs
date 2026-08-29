//! Patching one place a string is used, rather than the string (§18).
//!
//! Rewriting a Utf8 constant changes the text everywhere it appears. A game showing `Back` on
//! eleven screens has one constant for all eleven, and a translation that must differ on one of
//! them cannot be said in the pool at all. These tests are about saying it in the code instead,
//! and about the cases where doing so would break the class and is refused.

use tjlocalizer_core::classfile::ClassFile;

fn fixture() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/SampleGame.class"
    ))
    .expect("fixture missing - run tools/make-fixtures.sh")
}

#[test]
fn every_place_a_string_is_loaded_is_found() {
    let class = ClassFile::parse(&fixture()).unwrap();
    let sites = class.string_sites().unwrap();

    let loaded: Vec<String> = sites.iter().filter_map(|s| s.text.clone()).collect();
    for expected in [
        "Dragon Quest Online",
        "Start Game",
        "Quit",
        "You have arrived at last, traveller.",
    ] {
        assert!(
            loaded.contains(&expected.to_string()),
            "missing {expected:?}"
        );
    }

    // Every site is in a method the class actually has, at a position inside its code.
    for site in &sites {
        assert!(!site.method.is_empty(), "{site:?}");
        assert!(site.descriptor.starts_with('('), "{site:?}");
    }
    // The fixture shows the quit label from two methods, which is the case this module is for.
    let quit: Vec<&str> = sites
        .iter()
        .filter(|s| s.text.as_deref() == Some("Quit"))
        .map(|s| s.method.as_str())
        .collect();
    assert_eq!(quit, vec!["confirm", "main"]);
}

/// The point of the whole exercise: two uses of one string, translated apart.
#[test]
fn one_use_of_a_string_can_be_changed_without_changing_the_other() {
    let mut class = ClassFile::parse(&fixture()).unwrap();

    let sites = class.string_sites().unwrap();
    let quit = sites
        .iter()
        .find(|s| s.text.as_deref() == Some("Quit"))
        .cloned()
        .expect("the fixture should load Quit");

    let replacement = class.add_string("Thoát").unwrap();
    class.point_site_at(&quit, replacement).unwrap();
    let untouched = sites
        .iter()
        .filter(|s| s.text.as_deref() == Some("Quit"))
        .count()
        - 1;
    assert_eq!(untouched, 1, "the fixture should load Quit twice");

    let rebuilt = ClassFile::parse(&class.write().unwrap()).unwrap();
    let after = rebuilt.string_sites().unwrap();

    assert_eq!(
        after
            .iter()
            .filter(|s| s.text.as_deref() == Some("Thoát"))
            .count(),
        1,
        "exactly one of the two uses should have changed"
    );
    assert_eq!(
        after
            .iter()
            .filter(|s| s.text.as_deref() == Some("Quit"))
            .count(),
        1,
        "the other use should still load the original"
    );
    // The original constant is untouched, which is what makes this different from a pool rewrite.
    assert!(rebuilt
        .string_literals()
        .iter()
        .any(|l| l.decoded.as_deref() == Some("Quit")));
    // And everything else still loads what it loaded.
    for text in ["Dragon Quest Online", "Start Game"] {
        assert!(
            after.iter().any(|s| s.text.as_deref() == Some(text)),
            "{text} stopped being loaded"
        );
    }
}

/// The refusal that keeps this safe. Widening an `ldc` to an `ldc_w` moves every instruction
/// after it, and a method whose jumps are one byte out fails verification in a way nobody can
/// debug from a translated string.
#[test]
fn a_site_that_cannot_reach_a_new_constant_is_refused_rather_than_widened() {
    let mut class = ClassFile::parse(&fixture()).unwrap();
    let site = class
        .string_sites()
        .unwrap()
        .into_iter()
        .find(|s| s.text.as_deref() == Some("Quit"))
        .unwrap();

    // Fill the pool past what a one-byte operand can address.
    let mut last = 0;
    while class.constant_count() < 300 {
        last = class.add_string("padding").unwrap();
    }

    let refused = class.point_site_at(&site, last).unwrap_err();
    let message = refused.to_string();
    assert!(message.contains("ldc"), "{message}");
    assert!(message.contains("jump"), "{message}");

    // And the class is unchanged where it matters: the site still loads what it loaded.
    let after = ClassFile::parse(&class.write().unwrap()).unwrap();
    assert!(after
        .string_sites()
        .unwrap()
        .iter()
        .any(|s| s.text.as_deref() == Some("Quit")));
}

#[test]
fn pointing_a_site_at_a_constant_that_does_not_exist_is_refused() {
    let mut class = ClassFile::parse(&fixture()).unwrap();
    let site = class.string_sites().unwrap().into_iter().next().unwrap();
    let count = class.constant_count();
    assert!(class.point_site_at(&site, count + 50).is_err());
}
