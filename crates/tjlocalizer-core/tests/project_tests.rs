//! The project directory: immutability of the original, versioning, builds and rollback.

use tjlocalizer_core::lang::Language;
use tjlocalizer_core::project::{Project, DIRECTORIES, SCHEMA_VERSION};

fn vi() -> Language {
    Language::new("vi-VN")
}

fn fixture() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/sample-game.jar"
    ))
    .expect("fixture missing - run tools/make-fixtures.sh")
}

/// A temporary directory that cleans itself up, so the tests leave nothing behind.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tjlocalizer-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn new_project(tag: &str) -> (TempDir, Project) {
    let dir = TempDir::new(tag);
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    (dir, project)
}

#[test]
fn import_lays_out_the_directory_and_pins_the_original() {
    let (dir, project) = new_project("import");

    for name in DIRECTORIES {
        assert!(dir.0.join(name).is_dir(), "{name} should have been created");
    }
    assert_eq!(project.profile().schema_version, SCHEMA_VERSION);
    assert_eq!(project.profile().source.jar, "original/sample-game.jar");
    assert_eq!(project.profile().source.sha256.len(), 64);
    assert_eq!(project.profile().targets[0].language.tag(), "vi-VN");

    // The recorded hash must be the hash of the bytes that were handed in, not of a re-zip.
    assert_eq!(
        project.profile().source.sha256,
        tjlocalizer_core::jar::sha256_hex(&fixture())
    );
}

#[test]
fn a_second_import_into_the_same_directory_is_refused() {
    let (dir, _project) = new_project("double-import");
    let err = Project::create(&dir.0, "sample-game", &fixture()).unwrap_err();
    assert!(err.to_string().contains("already exists"));
}

#[test]
fn reopening_gives_back_the_same_profile() {
    let (dir, project) = new_project("reopen");
    let reopened = Project::open(&dir.0).unwrap();
    assert_eq!(reopened.profile().name, project.profile().name);
    assert_eq!(
        reopened.profile().source.sha256,
        project.profile().source.sha256
    );
}

#[test]
fn saving_bumps_the_revision() {
    let (dir, mut project) = new_project("revision");
    let first = project.profile().revision;
    project.profile_mut().targets[0].style_profile = "formal".to_string();
    project.save().unwrap();
    assert_eq!(project.profile().revision, first + 1);

    let reopened = Project::open(&dir.0).unwrap();
    assert_eq!(reopened.profile().revision, first + 1);
    assert_eq!(reopened.profile().targets[0].style_profile, "formal");
}

#[test]
fn a_modified_original_is_reported_rather_than_used() {
    let (dir, project) = new_project("tampered");
    std::fs::write(
        dir.0.join(&project.profile().source.jar),
        b"not a jar any more",
    )
    .unwrap();

    let err = Project::open(&dir.0).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("has been modified"), "got: {message}");
}

#[test]
fn a_newer_schema_is_refused_instead_of_half_read() {
    let (dir, _project) = new_project("schema");
    let path = dir.0.join("project.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let bumped = text.replace(
        &format!("\"schemaVersion\": {SCHEMA_VERSION}"),
        &format!("\"schemaVersion\": {}", SCHEMA_VERSION + 1),
    );
    assert_ne!(
        text, bumped,
        "the schema version should appear in project.json"
    );
    std::fs::write(&path, bumped).unwrap();

    let err = Project::open(&dir.0).unwrap_err();
    assert!(err.to_string().contains("understands up to"), "got: {err}");
}

#[test]
fn analyze_and_extract_persist_their_results() {
    let (dir, project) = new_project("analyze");

    let capabilities = project.analyze().unwrap();
    assert!(capabilities.has("midlet_entry"));
    assert!(dir.0.join("extracted/capabilities.json").exists());

    let graph = project.extract().unwrap();
    assert!(!graph.nodes.is_empty());
    assert!(dir.0.join("content/graph.json").exists());

    // Reading the graph back gives the same nodes, which is what lets translation happen in a
    // later session from a different process.
    let reloaded = project.graph().unwrap();
    assert_eq!(reloaded.nodes.len(), graph.nodes.len());
    assert_eq!(reloaded.nodes[0].id, graph.nodes[0].id);
}

#[test]
fn re_extracting_keeps_translations_attached_to_their_nodes() {
    let (_dir, project) = new_project("stable-ids");
    let graph = project.extract().unwrap();

    let node = graph
        .translatable()
        .into_iter()
        .find(|n| n.source_text == "Start Game")
        .expect("fixture should offer this literal");
    let mut store = project.translations(&vi()).unwrap();
    store.set(&node.id, "Bắt đầu trò chơi");
    project.save_translations(&vi(), &store).unwrap();

    let again = project.extract().unwrap();
    let same = again
        .translatable()
        .into_iter()
        .find(|n| n.source_text == "Start Game")
        .unwrap();
    assert_eq!(same.id, node.id);
    assert_eq!(
        project.translations(&vi()).unwrap().get(&same.id),
        Some("Bắt đầu trò chơi")
    );
}

#[test]
fn build_records_what_it_produced_and_publishes_the_output() {
    let (dir, project) = new_project("build");
    let graph = project.extract().unwrap();

    let mut store = project.translations(&vi()).unwrap();
    for node in graph.translatable() {
        if node.source_text == "Quit" {
            store.set(&node.id, "Thoát");
        }
    }
    project.save_translations(&vi(), &store).unwrap();

    let record = project.build(&vi()).unwrap();
    assert_eq!(record.revision, 1);
    assert_eq!(record.profile_revision, project.profile().revision);
    assert_eq!(record.source_sha256, project.profile().source.sha256);
    assert_eq!(record.translations_applied, 1);
    assert_eq!(record.report.literals_patched, 1);
    assert!(
        record.validation.is_ok(),
        "{:?}",
        record.validation.findings
    );

    let name = project.output_name(project.target(&vi()).unwrap());
    assert_eq!(name, "sample-game-vi-vn.jar");
    assert!(dir.0.join("output").join(&name).exists());
    assert!(dir.0.join("builds/vi-vn/0001").join(&name).exists());
    assert!(dir.0.join("builds/vi-vn/0001/build.json").exists());
}

#[test]
fn rollback_restores_an_earlier_build() {
    let (dir, project) = new_project("rollback");
    let graph = project.extract().unwrap();
    let quit = graph
        .translatable()
        .into_iter()
        .find(|n| n.source_text == "Quit")
        .unwrap();

    let mut store = project.translations(&vi()).unwrap();
    store.set(&quit.id, "Thoát");
    project.save_translations(&vi(), &store).unwrap();
    project.build(&vi()).unwrap();
    let good = std::fs::read(
        dir.0
            .join("output")
            .join(project.output_name(project.target(&vi()).unwrap())),
    )
    .unwrap();

    // A second build with a translation someone later regrets.
    store.set(&quit.id, "Chấm dứt phiên làm việc ngay lập tức");
    project.save_translations(&vi(), &store).unwrap();
    let second = project.build(&vi()).unwrap();
    assert_eq!(second.revision, 2);
    let regrettable = std::fs::read(
        dir.0
            .join("output")
            .join(project.output_name(project.target(&vi()).unwrap())),
    )
    .unwrap();
    assert_ne!(good, regrettable);

    let restored = project.rollback(&vi(), 1).unwrap();
    assert_eq!(restored.revision, 1);
    assert_eq!(
        std::fs::read(
            dir.0
                .join("output")
                .join(project.output_name(project.target(&vi()).unwrap()))
        )
        .unwrap(),
        good
    );

    // Rolling back does not throw the newer build away.
    assert_eq!(project.builds(&vi()).unwrap().len(), 2);
    assert!(dir.0.join("builds/vi-vn/0002/build.json").exists());
}

#[test]
fn rolling_back_to_a_build_that_never_happened_is_an_error() {
    let (_dir, project) = new_project("rollback-missing");
    project.extract().unwrap();
    let err = project.rollback(&vi(), 7).unwrap_err();
    assert!(err.to_string().contains("no build 7"), "got: {err}");
}

/// A project should be able to tell a person which images in their game might be the font.
///
/// Finding the glyph sheet by hand means opening a JAR and looking at every PNG, which is the
/// part of this work people get wrong. The tool ranks; it does not choose.
#[test]
fn the_font_sheet_is_offered_ahead_of_the_artwork() {
    use tjlocalizer_core::font::sheet::{Grid, Image};
    use tjlocalizer_core::jar::Archive;

    let cell = 12u32;
    let columns = 16u32;
    let characters: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    let rows = (characters.len() as u32).div_ceil(columns);
    let grid = Grid {
        cell_width: cell,
        cell_height: cell,
        columns,
        rows,
    };
    let mut font = Image::new(columns * cell, rows * cell);
    for (i, c) in characters.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let (ox, oy) = grid.cell_origin(i as u32);
        for y in 0..5u32 {
            for x in 0..6u32 {
                font.set(ox + 3 + x, oy + 3 + y, [230, 230, 230, 255]);
            }
        }
    }

    let mut art = Image::new(192, 132);
    for y in 0..132u32 {
        for x in 0..192u32 {
            let v = |m: u32, n: u32| ((x * m + y * n) % 256) as u8;
            art.set(x, y, [v(7, 3), v(3, 11), v(13, 5), 255]);
        }
    }

    let mut archive = Archive::read(&fixture()).unwrap();
    archive.insert("sky.png", art.encode_png().unwrap());
    archive.insert("font.png", font.encode_png().unwrap());

    let dir = TempDir::new("font-candidates");
    let project = Project::create(&dir.0, "sample-game", &archive.write().unwrap()).unwrap();

    let candidates = project.font_candidates().unwrap();
    let names: Vec<&str> = candidates.iter().map(|c| c.entry.as_str()).collect();
    assert_eq!(
        names,
        vec!["font.png", "sky.png"],
        "the glyph sheet should be offered first"
    );
    assert_eq!(candidates[0].grids[0].grid, grid);
}
