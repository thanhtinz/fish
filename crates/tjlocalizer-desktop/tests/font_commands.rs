//! The font tab, end to end through the commands the interface actually calls.
//!
//! The interesting failures here are not in the drawing - the core has tests for that - but in
//! the order things happen in: composing before a sheet is declared, choosing a typeface before
//! a folder, reading a coverage report for a project that has no font. Each of those is a click
//! somebody will make, and each has to produce a sentence rather than a panic.

use tjlocalizer_core::font::sheet::{Grid, Image};
use tjlocalizer_core::jar::Archive;
use tjlocalizer_desktop_lib::commands;
use tjlocalizer_desktop_lib::state::GridView;

const CELL: u32 = 12;
const COLUMNS: u32 = 16;

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tjlocalizer-desktop-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A game whose font sheet has room above the letters, so the marks have somewhere to go.
fn game_with_a_font() -> (Vec<u8>, Grid) {
    let characters: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    let rows = (characters.len() as u32).div_ceil(COLUMNS);
    let grid = Grid {
        cell_width: CELL,
        cell_height: CELL,
        columns: COLUMNS,
        rows,
    };
    let mut image = Image::new(COLUMNS * CELL, rows * CELL);
    for (i, c) in characters.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let (ox, oy) = grid.cell_origin(i as u32);
        let seed = (*c as u32).wrapping_mul(2_654_435_761);
        for y in 0..4u32 {
            for x in 0..6u32 {
                let edge = x == 0 || x == 5 || y == 0 || y == 3;
                let inked = edge || (seed >> ((y - 1) * 4 + (x - 1).min(3))) & 1 == 1;
                if inked {
                    image.set(ox + 3 + x, oy + 6 + y, [220, 200, 120, 255]);
                }
            }
        }
    }

    let base = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tjlocalizer-core/tests/data/sample-game.jar"
    ))
    .expect("fixture missing - run tools/make-fixtures.sh");
    let mut archive = Archive::read(&base).unwrap();
    archive.insert("font.png", image.encode_png().unwrap());
    (archive.write().unwrap(), grid)
}

fn project(tag: &str) -> (TempDir, String) {
    let dir = TempDir::new(tag);
    let (jar, _) = game_with_a_font();
    tjlocalizer_core::project::Project::create(&dir.0, "sample-game", &jar).unwrap();
    let path = dir.0.display().to_string();
    (dir, path)
}

#[test]
fn a_project_with_no_font_declared_says_so_rather_than_reporting_full_coverage() {
    let (_dir, path) = project("undeclared");
    let font = commands::font_status(path).unwrap();

    assert!(!font.declared);
    assert_eq!(font.covered, 0);
    assert_eq!(font.required, 134);
    // "Nobody has checked" and "it covers everything" must not look the same to the interface.
    assert!(font.problem.is_none());
}

#[test]
fn the_sheet_is_offered_with_its_own_grid_first_and_can_be_chosen() {
    let (_dir, path) = project("choose");
    let (_, real) = game_with_a_font();

    let candidates = commands::font_candidates(path.clone()).unwrap();
    let sheet = candidates
        .iter()
        .find(|c| c.entry == "font.png")
        .expect("the font was not offered");
    assert_eq!(sheet.grids[0].grid, GridView::from(real));
    assert!(sheet.image.starts_with("data:image/png;base64,"));

    let font =
        commands::set_font_sheet(path, "font.png".into(), sheet.grids[0].grid, None).unwrap();
    assert!(font.declared);
    assert_eq!(font.entry, "font.png");
    // A sheet of ASCII covers none of the 134 and can compose all of them: that is the whole
    // reason this tab exists, and it is what the interface has to be able to say.
    assert_eq!(font.covered, 0);
    assert_eq!(font.composable, 134);
}

#[test]
fn a_device_font_needs_nothing_composing() {
    let (_dir, path) = project("device");
    let font = commands::set_device_font(path.clone()).unwrap();

    assert!(font.device_font);
    assert_eq!(font.covered, font.required);
    assert!(font.missing.is_empty());

    // And forgetting it goes back to "unknown", not to "fine".
    let cleared = commands::clear_font(path).unwrap();
    assert!(!cleared.declared);
}

#[test]
fn composing_writes_a_sheet_and_says_it_is_not_installed() {
    let (dir, path) = project("compose");
    let (_, grid) = game_with_a_font();
    commands::set_font_sheet(path.clone(), "font.png".into(), GridView::from(grid), None).unwrap();

    let result = commands::compose_font(path.clone()).unwrap();
    assert!(dir.0.join("fonts/extended.png").is_file());
    assert!(result.image.starts_with("data:image/png;base64,"));
    assert!(
        result.added.chars().count() > 100,
        "only {} letters were added",
        result.added.chars().count()
    );
    assert!(result.typeface.is_none(), "no typeface was chosen");

    let preview = commands::font_preview(path, Some("Cá đã cắn câu".into()), Some(2)).unwrap();
    assert!(preview.starts_with("data:image/png;base64,"));
}

/// Every one of these is a button somebody can press in the wrong order.
#[test]
fn acting_before_the_font_is_declared_explains_itself() {
    let (_dir, path) = project("order");

    for reply in [
        commands::compose_font(path.clone()).map(|_| ()),
        commands::font_preview(path.clone(), None, None).map(|_| ()),
        commands::set_marks_font(path.clone(), Some("/nonexistent.ttf".into())).map(|_| ()),
        commands::scan_font_library(path, "/usr/share/fonts".into(), Some(1)).map(|_| ()),
    ] {
        let message = reply.expect_err("this should not have been possible yet");
        assert!(
            !message.is_empty() && !message.contains("panic"),
            "unhelpful message: {message:?}"
        );
    }
}

/// Measuring a real folder of real fonts, when the machine has one.
///
/// Skipped rather than failed where it does not, because a build machine without fonts installed
/// is a fact about the machine and not about this code.
#[test]
fn a_folder_of_fonts_is_measured_against_this_sheet() {
    let folder = std::path::Path::new("/usr/share/fonts/truetype");
    if !folder.is_dir() {
        eprintln!("skipped: {} is not there", folder.display());
        return;
    }
    let (_dir, path) = project("library");
    let (_, grid) = game_with_a_font();
    commands::set_font_sheet(path.clone(), "font.png".into(), GridView::from(grid), None).unwrap();

    let scan = commands::scan_font_library(path.clone(), folder.display().to_string(), Some(6))
        .expect("the scan failed");
    assert!(
        scan.found > 0,
        "no fonts were found in {}",
        folder.display()
    );
    assert!(scan.measured <= 6, "the limit was not respected");
    assert!(
        scan.covering <= scan.found,
        "more fonts cover Vietnamese than exist"
    );

    // Choosing the folder is remembered; choosing a font from it is separate, because a person
    // will want to look at several before deciding.
    let after = commands::font_status(path.clone()).unwrap();
    assert_eq!(
        after.mark_library.as_deref(),
        Some(folder.to_str().unwrap())
    );
    assert!(after.marks_from.is_none());

    if let Some(best) = scan.fonts.first() {
        let chosen = commands::set_marks_font(path.clone(), Some(best.path.clone())).unwrap();
        assert_eq!(chosen.marks_from.as_deref(), Some(best.path.as_str()));

        let composed = commands::compose_font(path.clone()).unwrap();
        assert_eq!(composed.typeface.as_deref(), Some(best.path.as_str()));

        // And back to the drawn marks, which has to be possible: a typeface supplying more marks
        // is not a typeface producing better ones at twelve pixels.
        let drawn = commands::set_marks_font(path, None).unwrap();
        assert!(drawn.marks_from.is_none());
    }
}
