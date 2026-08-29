//! A game that sits on disk as a directory rather than in one file.
//!
//! The claim this file has to prove is a proportion, not a feature: a game install is tens of
//! thousands of files, almost all of them textures and audio, and what makes the directory path
//! usable is that it reads three of them. So most of what is asserted here is what was *not*
//! read - and a self-made tree proves that completely, because every file in it was put there by
//! this test.

use std::path::Path;
use tjlocalizer_core::package::Kind;
use tjlocalizer_core::project::{Project, Source};
use tjlocalizer_core::tree::{self, Limits};

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tjlocalizer-tree-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(root: &Path, path: &str, data: &[u8]) {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, data).unwrap();
}

/// A game shaped the way one actually is: mostly things nobody translates.
fn game(root: &Path) {
    for i in 0..200 {
        write(root, &format!("Content/Textures/tex{i:04}.png"), &[0u8; 64]);
        write(root, &format!("Content/Audio/vo{i:04}.ogg"), &[0u8; 64]);
    }
    write(root, "Engine/Binaries/Win64/Game.exe", b"MZ\x00\x00");
    write(root, "steam_api64.dll", &[0u8; 32]);

    write(
        root,
        "Content/Localization/Game/en/Game.po",
        b"# a comment\nmsgid \"Start Game\"\nmsgstr \"\"\n",
    );
    write(
        root,
        "Content/dialogue.json",
        br#"{"lines":[{"text":"You caught a fish!"}]}"#,
    );
    write(
        root,
        "Content/settings.ini",
        b"; menus\n[menu]\ntitle=Options\n",
    );
}

/// The proportion is the feature. Four hundred and four files in, three read.
#[test]
fn a_game_directory_is_scanned_whole_and_read_selectively() {
    let dir = TempDir::new("scan");
    let source = dir.0.join("game");
    game(&source);

    let scan = tree::scan(&source, &Limits::default());
    assert_eq!(scan.files.len(), 405, "the scan should see everything");

    let ingested = tree::ingest(&source, scan, &Limits::default()).unwrap();
    assert_eq!(ingested.scanned, 405);
    assert_eq!(ingested.files.len(), 3, "{:?}", ingested.files);

    let names: Vec<&str> = ingested.files.iter().map(|f| f.path.as_str()).collect();
    assert!(names.contains(&"Content/dialogue.json"));
    assert!(names.contains(&"Content/settings.ini"));
    assert!(names.contains(&"Content/Localization/Game/en/Game.po"));
}

/// Textures are not listed as skipped. Four hundred lines saying "this png is not text" would
/// bury the one line that matters, which is the next test.
#[test]
fn files_nobody_would_expect_to_be_read_are_not_listed_as_skipped() {
    let dir = TempDir::new("quiet");
    let source = dir.0.join("game");
    game(&source);

    let scan = tree::scan(&source, &Limits::default());
    let ingested = tree::ingest(&source, scan, &Limits::default()).unwrap();
    assert!(ingested.skipped.is_empty(), "{:?}", ingested.skipped);
}

/// But a text file passed over for its size is listed, with its size and the reason. A 300 MB
/// JSON quietly dropped is exactly the thing a translator finds out about far too late.
#[test]
fn a_text_file_too_large_to_read_is_named_and_the_reason_given() {
    let dir = TempDir::new("large");
    let source = dir.0.join("game");
    game(&source);
    write(
        &source,
        "Content/telemetry.json",
        &vec![b'x'; 3 * 1024 * 1024],
    );

    let limits = Limits {
        max_file_size: 1024 * 1024,
        ..Limits::default()
    };
    let scan = tree::scan(&source, &limits);
    let ingested = tree::ingest(&source, scan, &limits).unwrap();

    assert_eq!(ingested.skipped.len(), 1, "{:?}", ingested.skipped);
    let skipped = &ingested.skipped[0];
    assert_eq!(skipped.path, "Content/telemetry.json");
    assert_eq!(skipped.size, 3 * 1024 * 1024);
    assert!(skipped.reason.contains("larger"), "{}", skipped.reason);
}

/// The engine falls out of the file names for free, and arrives as evidence rather than as a
/// verdict - so a wrong answer can be argued with.
#[test]
fn the_engine_is_guessed_from_names_and_reported_as_evidence() {
    let dir = TempDir::new("engine");
    let source = dir.0.join("game");
    game(&source);

    let scan = tree::scan(&source, &Limits::default());
    assert!(
        scan.evidence.iter().any(|e| e.contains("Steam")),
        "{:?}",
        scan.evidence
    );
    assert!(
        scan.evidence.iter().any(|e| e.contains("Unreal")),
        "{:?}",
        scan.evidence
    );
}

/// The whole point of ingestion: what comes out is an `Archive`, so the twenty-six functions that
/// take one - detection, extraction, the build, rules, validation - run on a directory unchanged.
#[test]
fn a_directory_project_extracts_text_through_the_ordinary_pipeline() {
    let dir = TempDir::new("pipeline");
    let source = dir.0.join("game");
    game(&source);

    let (project, _) =
        Project::create_from_tree(dir.0.join("p"), "fishing", &source, &Limits::default()).unwrap();

    let package = project.package().unwrap();
    // The variant that has been in the enum since the beginning and never once been produced.
    assert_eq!(package.kind, Kind::Directory);
    assert_eq!(package.readable.len(), 3, "{:?}", package.readable);

    let graph = project.extract().unwrap();
    let texts: Vec<&str> = graph.nodes.iter().map(|n| n.source_text.as_str()).collect();
    assert!(texts.contains(&"Start Game"), "{texts:?}");
    assert!(texts.contains(&"You caught a fish!"), "{texts:?}");
    assert!(texts.contains(&"Options"), "{texts:?}");
}

/// The originals are copied, not only hashed - so the project still holds the bytes it started
/// from after the game has been updated over, moved, or uninstalled.
#[test]
fn the_files_it_read_are_copied_into_the_project() {
    let dir = TempDir::new("pinned");
    let source = dir.0.join("game");
    game(&source);

    let (project, _) =
        Project::create_from_tree(dir.0.join("p"), "fishing", &source, &Limits::default()).unwrap();

    let pinned = project.root().join("original/tree/Content/dialogue.json");
    assert!(pinned.exists(), "the original was not copied");

    // Delete the game entirely; the project still opens and still knows its text.
    std::fs::remove_dir_all(&source).unwrap();
    let reopened = Project::open(project.root()).unwrap();
    assert_eq!(reopened.extract().unwrap().nodes.len(), 3);
}

/// A tree has no bytes of its own to hash, so it is pinned by a manifest of what was read. The
/// check has to catch a copy being edited, or "the original is immutable" is a claim and not a
/// property.
#[test]
fn editing_a_pinned_original_is_caught_on_open() {
    let dir = TempDir::new("verify");
    let source = dir.0.join("game");
    game(&source);

    let (project, _) =
        Project::create_from_tree(dir.0.join("p"), "fishing", &source, &Limits::default()).unwrap();
    let root = project.root().to_path_buf();
    assert!(Project::open(&root).is_ok());

    std::fs::write(root.join("original/tree/Content/settings.ini"), b"tampered").unwrap();
    let err = Project::open(&root).unwrap_err().to_string();
    assert!(err.contains("modified"), "{err}");
}

/// The files that were not read were not hashed either, and the record says so rather than
/// leaving somebody to assume a whole-game guarantee that was never made.
#[test]
fn the_record_admits_that_unread_files_are_not_hashed() {
    let dir = TempDir::new("record");
    let source = dir.0.join("game");
    game(&source);

    let (project, _) =
        Project::create_from_tree(dir.0.join("p"), "fishing", &source, &Limits::default()).unwrap();
    let record = project.tree_record().unwrap();

    assert!(record.unread_files_are_not_hashed);
    assert_eq!(record.scanned, 405);
    assert_eq!(record.files.len(), 3);
}

/// A directory with nothing readable in it is an error at import, not a project that opens and
/// shows an empty table. The second is the same outcome with a day wasted first.
#[test]
fn a_directory_with_no_readable_files_is_refused_at_import() {
    let dir = TempDir::new("empty");
    let source = dir.0.join("game");
    write(&source, "Content/tex.png", &[0u8; 64]);

    let err = Project::create_from_tree(dir.0.join("p"), "x", &source, &Limits::default())
        .unwrap_err()
        .to_string();
    assert!(err.contains("format this build can read"), "{err}");
}

/// The source is a tagged union on disk. Untagged would let a project file match the wrong
/// variant, which is the kind of bug that eats somebody's work without saying anything.
#[test]
fn the_source_kind_is_written_out_and_read_back() {
    let dir = TempDir::new("tag");
    let source = dir.0.join("game");
    game(&source);

    let (project, _) =
        Project::create_from_tree(dir.0.join("p"), "fishing", &source, &Limits::default()).unwrap();
    let text = std::fs::read_to_string(project.root().join("project.json")).unwrap();
    assert!(text.contains("\"kind\": \"tree\""), "{text}");

    assert!(matches!(
        Project::open(project.root()).unwrap().profile().source,
        Source::Tree { .. }
    ));
}

/// A project written before directories existed has no `kind` on its source, and has to keep
/// opening. A migration that dropped it would take the translator's work with it.
#[test]
fn a_project_from_before_directories_still_opens() {
    let dir = TempDir::new("v3");
    let source = dir.0.join("game");
    game(&source);
    let (project, _) =
        Project::create_from_tree(dir.0.join("p"), "fishing", &source, &Limits::default()).unwrap();
    let root = project.root().to_path_buf();

    // Rewrite project.json the way version 3 wrote it: an untagged source, schema 3.
    let mut raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("project.json")).unwrap()).unwrap();
    let sha = raw["source"]["sha256"].clone();
    raw["schemaVersion"] = serde_json::json!(3);
    raw["source"] = serde_json::json!({ "jar": "original/fishing.jar", "sha256": sha });
    std::fs::write(
        root.join("project.json"),
        serde_json::to_string_pretty(&raw).unwrap(),
    )
    .unwrap();
    // And give it the file that version 3 would have had.
    std::fs::write(root.join("original/fishing.jar"), b"not really a jar").unwrap();

    // It fails on the hash, not on the shape - which is the point: it was understood, then
    // checked.
    let err = Project::open(&root).unwrap_err().to_string();
    assert!(err.contains("modified"), "{err}");
}

/// A directory game builds to a directory, not to a file with no extension that nothing can
/// install. `output_name` is derived in one place and used by rollback, export and the interface,
/// so getting it wrong here is wrong everywhere.
#[test]
fn a_directory_game_builds_to_a_directory_rather_than_a_file() {
    let dir = TempDir::new("build");
    let source = dir.0.join("game");
    game(&source);

    let (project, _) =
        Project::create_from_tree(dir.0.join("p"), "fishing", &source, &Limits::default()).unwrap();
    let target = project
        .target(&tjlocalizer_core::lang::Language::new("vi-VN"))
        .unwrap();
    let name = project.output_name(target);
    assert_eq!(name, "fishing-vi-vn");
    assert!(
        !name.contains('.'),
        "a folder was named as though it were a file"
    );
}

// -- the patch ----------------------------------------------------------------------------------

fn translated(project: &Project) -> tjlocalizer_core::lang::Language {
    let language = tjlocalizer_core::lang::Language::new("vi-VN");
    let graph = project.extract().unwrap();
    let want = [
        ("Start Game", "Bắt đầu"),
        ("You caught a fish!", "Bạn câu được một con cá!"),
        ("Options", "Tuỳ chọn"),
    ];
    let mut store = tjlocalizer_core::translation::TranslationStore::default();
    for node in &graph.nodes {
        if let Some((_, target)) = want.iter().find(|(s, _)| *s == node.source_text) {
            store.set(&node.id, *target);
        }
    }
    project.save_translations(&language, &store).unwrap();
    language
}

fn built(tag: &str) -> (TempDir, std::path::PathBuf, Project) {
    let dir = TempDir::new(tag);
    let source = dir.0.join("game");
    game(&source);
    let (project, _) =
        Project::create_from_tree(dir.0.join("p"), "fishing", &source, &Limits::default()).unwrap();
    let language = translated(&project);
    project.build(&language).unwrap();
    (dir, source, project)
}

/// The whole reason a directory build is different: it writes what changed, not a second copy of
/// somebody's game. Four hundred files in, three out.
#[test]
fn a_directory_build_writes_only_the_files_it_changed() {
    let (_dir, _source, project) = built("patch");
    let patch = project
        .patch_dir(
            project
                .target(&tjlocalizer_core::lang::Language::new("vi-VN"))
                .unwrap(),
        )
        .expect("no patch was written");

    assert!(patch.join("Content/dialogue.json").exists());
    assert!(patch.join("Content/settings.ini").exists());
    assert!(patch.join("patch.json").exists());
    assert!(patch.join("INSTALL.txt").exists());

    // Not a texture in sight.
    assert!(!patch.join("Content/Textures/tex0000.png").exists());
    assert!(!patch.join("Engine/Binaries/Win64/Game.exe").exists());
}

/// Attribution travels with the patch rather than being written into the game. A stray file in an
/// install directory is something a game's own integrity check may notice, and the patch is what
/// gets sent to another person anyway.
#[test]
fn nothing_is_added_to_the_game_that_was_not_already_in_it() {
    let (_dir, source, project) = built("branding");
    let patch = project
        .patch_dir(
            project
                .target(&tjlocalizer_core::lang::Language::new("vi-VN"))
                .unwrap(),
        )
        .unwrap();
    let manifest = tjlocalizer_core::patch::read(&patch).unwrap();

    for change in &manifest.changes {
        assert!(
            source.join(&change.path).exists(),
            "{} is not a file the game already had",
            change.path
        );
    }
    assert!(
        !patch.join("META-INF").exists(),
        "META-INF reached a folder game"
    );
    // But the attribution is in the patch, where somebody receiving it will read it.
    assert_eq!(manifest.localized_by.as_deref(), Some("Thanhtinz"));
    let install = std::fs::read_to_string(patch.join("INSTALL.txt")).unwrap();
    assert!(install.contains("Thanhtinz"), "{install}");
}

/// Applying replaces exactly the files the patch names and nothing else, and what it replaced can
/// be put back.
#[test]
fn applying_a_patch_changes_only_the_named_files_and_keeps_what_it_replaced() {
    let (dir, source, project) = built("apply");
    let language = tjlocalizer_core::lang::Language::new("vi-VN");

    // Onto a copy, never the imported game: a test that patches its own fixture proves less.
    let copy = dir.0.join("copy");
    copy_dir(&source, &copy);

    let written = project.apply_patch(&language, &copy).unwrap();
    assert_eq!(written.len(), 3, "{written:?}");

    let patched = std::fs::read_to_string(copy.join("Content/settings.ini")).unwrap();
    assert!(patched.contains("Tuỳ chọn"), "{patched}");

    // Everything else came through untouched. This is the assertion a self-made tree proves
    // completely: every file in it was put there by this test.
    let mut differing = Vec::new();
    for entry in walkdir::WalkDir::new(&source)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry.path().strip_prefix(&source).unwrap();
        let theirs = std::fs::read(copy.join(relative)).unwrap();
        if std::fs::read(entry.path()).unwrap() != theirs {
            differing.push(relative.to_string_lossy().to_string());
        }
    }
    differing.sort();
    assert_eq!(
        differing,
        vec![
            "Content/Localization/Game/en/Game.po",
            "Content/dialogue.json",
            "Content/settings.ini",
        ]
    );

    // And the backup holds what was overwritten, byte for byte.
    let backup = project
        .root()
        .join("builds/vi-vn/0001/backup/Content/settings.ini");
    assert_eq!(
        std::fs::read(&backup).unwrap(),
        std::fs::read(source.join("Content/settings.ini")).unwrap()
    );
}

/// A patch built from one copy of a game must not be written over another. Half a translation is
/// worse than none: the game is then in a state neither the patch nor the backup describes.
#[test]
fn a_patch_is_refused_whole_when_the_game_is_not_the_one_it_was_built_from() {
    let (dir, source, project) = built("refuse");
    let language = tjlocalizer_core::lang::Language::new("vi-VN");

    let copy = dir.0.join("copy");
    copy_dir(&source, &copy);
    std::fs::write(copy.join("Content/settings.ini"), b"[menu]\ntitle=Edited\n").unwrap();

    let before = std::fs::read(copy.join("Content/dialogue.json")).unwrap();
    let err = project
        .apply_patch(&language, &copy)
        .unwrap_err()
        .to_string();
    assert!(err.contains("settings.ini"), "{err}");
    assert!(err.contains("none of it was applied"), "{err}");

    // The file that *would* have applied cleanly was not written either.
    assert_eq!(
        std::fs::read(copy.join("Content/dialogue.json")).unwrap(),
        before,
        "a file was written despite the patch being refused"
    );
}

/// Applying the same patch twice is refused, and says why in a way that is not alarming. A game
/// that already holds the translation is not a broken game.
#[test]
fn a_patch_already_applied_says_so_rather_than_reporting_damage() {
    let (dir, source, project) = built("twice");
    let language = tjlocalizer_core::lang::Language::new("vi-VN");
    let copy = dir.0.join("copy");
    copy_dir(&source, &copy);

    project.apply_patch(&language, &copy).unwrap();
    let plan = project.plan_patch(&language, &copy).unwrap();
    assert!(!plan.is_applicable());
    assert!(
        plan.mismatched.iter().all(|m| m.reason.contains("already")),
        "{:?}",
        plan.mismatched
    );
}

/// Steam updating over a game is the quietest way this work goes wrong: nothing errors, the patch
/// simply stops fitting. So it has a name a person can look up.
#[test]
fn a_game_updated_since_import_is_reported_as_drift() {
    let (_dir, source, project) = built("drift");
    assert!(
        project.check_drift().unwrap().is_empty(),
        "clean tree drifted"
    );

    std::fs::write(
        source.join("Content/dialogue.json"),
        br#"{"lines":[{"text":"You caught a bigger fish!"}]}"#,
    )
    .unwrap();

    let findings = project.check_drift().unwrap();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].check, "tree.drift");
    assert!(
        findings[0].detail.contains("dialogue.json"),
        "{:?}",
        findings[0]
    );
    assert!(findings[0].detail.contains("updated"), "{:?}", findings[0]);
}

/// A game that has moved or been uninstalled is not an error - the project still holds its own
/// copies - but it is worth saying, because nothing could be compared.
#[test]
fn a_game_that_is_no_longer_there_is_said_rather_than_ignored() {
    let (_dir, source, project) = built("gone");
    std::fs::remove_dir_all(&source).unwrap();

    let findings = project.check_drift().unwrap();
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].detail.contains("no longer"),
        "{:?}",
        findings[0]
    );
    assert!(
        findings[0].detail.contains("its own copies"),
        "{:?}",
        findings[0]
    );
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in walkdir::WalkDir::new(from)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let destination = to.join(entry.path().strip_prefix(from).unwrap());
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::copy(entry.path(), destination).unwrap();
    }
}
