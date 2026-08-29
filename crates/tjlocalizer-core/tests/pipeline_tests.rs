//! The whole pipeline: read a JAR, translate it, rebuild it, validate the result.

use tjlocalizer_core::build::{apply, Branding};
use tjlocalizer_core::graph::{self, ContextType};
use tjlocalizer_core::jar::{Archive, Manifest};
use tjlocalizer_core::lang::Language;
use tjlocalizer_core::translation::TranslationStore;
use tjlocalizer_core::validate::{validate, Severity, Subject};

fn original() -> Archive {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/sample-game.jar"
    ))
    .expect("fixture missing - run tools/make-fixtures.sh");
    Archive::read(&bytes).unwrap()
}

/// Translates everything translatable, so the build exercises both class and resource patching.
fn translate_all(graph: &graph::ContentGraph) -> TranslationStore {
    let mut store = TranslationStore::default();
    for node in graph.translatable() {
        let target = match node.source_text.as_str() {
            "Dragon Quest Online" => "Truyền Kỳ Rồng Thiêng",
            "Start Game" => "Bắt đầu trò chơi",
            "Quit" => "Thoát",
            "You have arrived at last, traveller." => "Rốt cuộc ngươi cũng tới rồi, lữ khách.",
            "HP: %d / %d" => "Sinh lực: %d / %d",
            "装备" => "Trang bị",
            "Green Field" => "Đồng Xanh",
            "Find the key" => "Tìm chìa khoá",
            _ => continue,
        };
        store.set(&node.id, target);
    }
    store
}

#[test]
fn translates_rebuilds_and_validates() {
    let original = original();
    let graph = graph::extract(&original);
    let store = translate_all(&graph);
    assert!(
        store.len() >= 6,
        "fixture should yield several translations"
    );

    let (built, report) = apply(&original, &graph, &store, &Branding::default()).unwrap();
    assert!(report.classes_patched >= 1);
    assert!(report.literals_patched >= 5);
    assert!(report.resources_patched >= 2);
    assert_eq!(report.output_sha256.len(), 64);

    let validation = validate(&Subject::new(
        &original,
        &built,
        &graph,
        &store,
        &Language::new("en"),
        &Language::new("vi-VN"),
    ));
    for finding in validation.errors() {
        eprintln!("unexpected error: {} - {}", finding.check, finding.detail);
    }
    assert!(validation.is_ok(), "validation reported errors");
}

#[test]
fn the_rebuilt_archive_still_holds_every_original_entry() {
    let original = original();
    let graph = graph::extract(&original);
    let store = translate_all(&graph);
    let (built, _) = apply(&original, &graph, &store, &Branding::default()).unwrap();

    for entry in original.entries() {
        assert!(built.get(&entry.name).is_some(), "{} was lost", entry.name);
    }
}

#[test]
fn translated_text_is_present_and_english_is_gone() {
    let original = original();
    let graph = graph::extract(&original);
    let store = translate_all(&graph);
    let (built, _) = apply(&original, &graph, &store, &Branding::default()).unwrap();

    let rebuilt_graph = graph::extract(&built);
    let texts: Vec<&str> = rebuilt_graph
        .nodes
        .iter()
        .map(|n| n.source_text.as_str())
        .collect();

    assert!(texts.contains(&"Bắt đầu trò chơi"));
    assert!(texts.contains(&"Đồng Xanh"));
    assert!(!texts.contains(&"Start Game"));
    assert!(!texts.contains(&"Green Field"));
    // Technical strings must come through untouched.
    assert!(texts.contains(&"/img/hud.png"));
}

#[test]
fn branding_is_added_without_touching_the_original_manifest() {
    let original = original();
    let graph = graph::extract(&original);
    let (built, _) = apply(
        &original,
        &graph,
        &TranslationStore::default(),
        &Branding::default(),
    )
    .unwrap();

    assert!(built.get("META-INF/THANHTINZ.BRAND").is_some());
    assert!(built.get("META-INF/LOCALIZATION.MF").is_some());

    // §36: attribution covers the localization, never the game. Every original attribute has to
    // survive byte for byte.
    let before = Manifest::parse(&String::from_utf8_lossy(
        &original.get("META-INF/MANIFEST.MF").unwrap().data,
    ));
    let after = Manifest::parse(&String::from_utf8_lossy(
        &built.get("META-INF/MANIFEST.MF").unwrap().data,
    ));
    for (key, value) in before.iter() {
        assert_eq!(after.get(key), Some(value.as_str()), "{key} changed");
    }
}

#[test]
fn branding_can_be_switched_off() {
    let original = original();
    let graph = graph::extract(&original);
    let branding = Branding {
        enabled: false,
        ..Default::default()
    };
    let (built, _) = apply(&original, &graph, &TranslationStore::default(), &branding).unwrap();
    assert!(built.get("META-INF/THANHTINZ.BRAND").is_none());
}

#[test]
fn validation_catches_a_lost_placeholder() {
    let original = original();
    let graph = graph::extract(&original);
    let node = graph
        .nodes
        .iter()
        .find(|n| n.context == ContextType::Format)
        .expect("fixture has a format string");

    let mut store = TranslationStore::default();
    store.set(&node.id, "Sinh lực đầy đủ"); // both %d dropped

    let (built, _) = apply(&original, &graph, &store, &Branding::default()).unwrap();
    let report = validate(&Subject::new(
        &original,
        &built,
        &graph,
        &store,
        &Language::new("en"),
        &Language::new("vi-VN"),
    ));

    assert!(
        !report.is_ok(),
        "a dropped placeholder must fail validation"
    );
    assert!(report
        .findings
        .iter()
        .any(|f| f.check == "translation.placeholder" && f.severity == Severity::Error));
}

#[test]
fn validation_catches_a_missing_entry_point() {
    let original = original();
    let graph = graph::extract(&original);
    let (mut built, _) = apply(
        &original,
        &graph,
        &TranslationStore::default(),
        &Branding::default(),
    )
    .unwrap();

    // The manifest still names SampleGame as the MIDlet, but the class is gone: the archive
    // installs and then fails to start. Without this check the build would be reported as clean.
    let manifest = Manifest::parse(&String::from_utf8_lossy(
        &built.get("META-INF/MANIFEST.MF").unwrap().data,
    ));
    assert_eq!(manifest.midlet_classes(), vec!["SampleGame".to_string()]);
    assert!(built.remove("SampleGame.class"));

    let report = validate(&Subject::new(
        &original,
        &built,
        &graph,
        &TranslationStore::default(),
        &Language::new("en"),
        &Language::new("vi-VN"),
    ));
    let entry_point = report
        .errors()
        .find(|f| f.check == "entry_point")
        .expect("the missing MIDlet class should be reported");
    assert!(entry_point.detail.contains("SampleGame"));
}

/// One label translated two ways, and two labels translated the same way (§24).
///
/// Neither is visible to any other check: each translation on its own is the right length, in the
/// right script, with its placeholders intact. It is only across the game that they are wrong.
#[test]
fn validation_catches_a_label_translated_two_ways() {
    let original = original();
    let graph = graph::extract(&original);

    // The fixture's own strings, translated inconsistently: the same source text given two
    // different words in two places is what a player reads as two different buttons.
    let labels: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.context == ContextType::Ui)
        .collect();
    assert!(labels.len() >= 2, "the fixture should have short labels");

    let mut store = TranslationStore::default();
    store.set(&labels[0].id, "Quay lại");
    store.set(&labels[1].id, "Quay lại");

    let (built, _) = apply(&original, &graph, &store, &Branding::default()).unwrap();
    let report = validate(&Subject::new(
        &original,
        &built,
        &graph,
        &store,
        &Language::new("en"),
        &Language::new("vi-VN"),
    ));

    let merged = report
        .findings
        .iter()
        .find(|f| f.check == "consistency.merged")
        .expect("two labels reading the same should be reported");
    assert_eq!(merged.severity, Severity::Warning, "it is a judgement");
    assert!(merged.detail.contains("Quay lại"));

    // A judgement, not a failure: the build still ships.
    assert!(report.is_ok());
}
