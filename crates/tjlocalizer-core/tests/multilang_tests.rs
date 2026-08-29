//! Several target languages from one project, and the migration that got us here.

use tjlocalizer_core::lang::Language;
use tjlocalizer_core::project::{Project, Target, SCHEMA_VERSION};

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
        let path = std::env::temp_dir().join(format!("tjlocalizer-ml-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn project(tag: &str) -> (TempDir, Project) {
    let dir = TempDir::new(tag);
    let project = Project::create(&dir.0, "game", &fixture()).unwrap();
    (dir, project)
}

fn lang(tag: &str) -> Language {
    Language::new(tag)
}

#[test]
fn a_new_project_starts_with_one_target_and_a_detected_source() {
    let (_dir, project) = project("new");
    assert_eq!(project.profile().targets.len(), 1);
    assert_eq!(project.profile().targets[0].language.tag(), "vi-VN");
    assert!(
        project.profile().source_language.detected,
        "the source language was guessed and must be recorded as a guess"
    );
}

#[test]
fn languages_can_be_added_and_each_keeps_its_own_work() {
    let (_dir, mut project) = project("add");
    project.extract().unwrap();
    for tag in ["en", "th", "id"] {
        project.add_target(lang(tag), "en-plain").unwrap();
    }
    assert_eq!(project.active_targets().len(), 4);

    let node = project
        .graph()
        .unwrap()
        .translatable()
        .find(|n| n.source_text == "Quit")
        .unwrap()
        .id
        .clone();

    let mut vi = project.translations(&lang("vi-VN")).unwrap();
    vi.set(&node, "Thoát");
    project.save_translations(&lang("vi-VN"), &vi).unwrap();

    let mut th = project.translations(&lang("th")).unwrap();
    th.set(&node, "ออก");
    project.save_translations(&lang("th"), &th).unwrap();

    // One language's work must not appear in another's.
    assert_eq!(
        project.translations(&lang("vi-VN")).unwrap().get(&node),
        Some("Thoát")
    );
    assert_eq!(
        project.translations(&lang("th")).unwrap().get(&node),
        Some("ออก")
    );
    assert_eq!(project.translations(&lang("id")).unwrap().get(&node), None);
}

#[test]
fn a_language_the_project_does_not_have_is_refused_rather_than_created() {
    let (_dir, project) = project("missing");
    let err = project.translations(&lang("ko")).unwrap_err();
    assert!(err.to_string().contains("no ko target"), "{err}");
}

#[test]
fn each_language_builds_to_its_own_file_and_history() {
    let (dir, mut project) = project("build");
    project.extract().unwrap();
    project.add_target(lang("en"), "en-plain").unwrap();

    let records = project.build_all().unwrap();
    assert_eq!(records.len(), 2);
    for record in &records {
        assert!(
            record.validation.is_ok(),
            "{:?}",
            record.validation.findings
        );
        assert_eq!(record.revision, 1);
    }

    assert!(dir.0.join("output/game-vi-vn.jar").exists());
    assert!(dir.0.join("output/game-en.jar").exists());
    assert!(dir.0.join("builds/vi-vn/0001/build.json").exists());
    assert!(dir.0.join("builds/en/0001/build.json").exists());

    // A build in one language must not advance another's revision counter.
    project.build(&lang("en")).unwrap();
    assert_eq!(project.builds(&lang("en")).unwrap().len(), 2);
    assert_eq!(project.builds(&lang("vi-VN")).unwrap().len(), 1);
}

/// Removing a language must not throw away reviewed work: re-adding it picks the work back up.
#[test]
fn removing_a_language_keeps_its_translations() {
    let (_dir, mut project) = project("remove");
    project.extract().unwrap();
    let node = project
        .graph()
        .unwrap()
        .translatable()
        .find(|n| n.source_text == "Quit")
        .unwrap()
        .id
        .clone();

    let mut store = project.translations(&lang("vi-VN")).unwrap();
    store.set(&node, "Thoát");
    project.save_translations(&lang("vi-VN"), &store).unwrap();

    project.remove_target(&lang("vi-VN")).unwrap();
    assert!(project.target(&lang("vi-VN")).is_none());

    project
        .add_target(lang("vi-VN"), "natural-dialogue")
        .unwrap();
    assert_eq!(
        project.translations(&lang("vi-VN")).unwrap().get(&node),
        Some("Thoát")
    );
}

/// A project written before multi-language support must open, keep its target and its register,
/// and be rewritten in the new shape - not refused, and not silently emptied.
#[test]
fn a_schema_2_project_is_migrated_rather_than_refused() {
    let dir = TempDir::new("migrate");
    let project = Project::create(&dir.0, "game", &fixture()).unwrap();
    let sha = project.profile().source.sha256().to_string();
    drop(project);

    let legacy = serde_json::json!({
        "schemaVersion": 2,
        "name": "game",
        "revision": 7,
        "source": { "jar": "original/game.jar", "sha256": sha },
        "localization": {
            "sourceLanguage": "auto",
            "targetLanguage": "vi-VN",
            "styleProfile": "formal"
        },
        "branding": { "enabled": true, "author": "Thanhtinz",
                      "localization_version": "1.0.0", "year": "2026" }
    });
    std::fs::write(
        dir.0.join("project.json"),
        serde_json::to_string_pretty(&legacy).unwrap(),
    )
    .unwrap();

    let reopened = Project::open(&dir.0).unwrap();
    assert_eq!(reopened.profile().schema_version, SCHEMA_VERSION);
    assert_eq!(reopened.profile().targets.len(), 1);
    assert_eq!(reopened.profile().targets[0].language.tag(), "vi-VN");
    assert_eq!(
        reopened.profile().targets[0].style_profile,
        "formal",
        "the register the project chose must survive migration"
    );
    // "auto" was version 2's way of saying "not decided"; it becomes an undetermined tag rather
    // than a guess presented as a fact.
    assert_eq!(reopened.profile().source_language.language.tag(), "und");
    assert!(reopened.profile().source_language.detected);

    // Rewritten in the new shape on disk, so the migration happens once.
    let on_disk = std::fs::read_to_string(dir.0.join("project.json")).unwrap();
    assert!(on_disk.contains("\"targets\""));
    assert!(!on_disk.contains("\"localization\""));
}

#[test]
fn a_target_slug_is_safe_as_a_file_name() {
    for tag in ["vi-VN", "zh-Hans", "pt-BR", "en"] {
        let slug = Target::new(Language::new(tag), "x").slug();
        assert!(
            slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "{tag} gave {slug}"
        );
    }
}
