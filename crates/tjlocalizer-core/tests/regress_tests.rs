//! Comparing one drawing of a game's text against the last one somebody accepted (§25).

use tjlocalizer_core::font::sheet::Image;
use tjlocalizer_core::regress::{compare, marked, Emulator};

fn filled(width: u32, height: u32, colour: [u8; 4]) -> Image {
    let mut image = Image::new(width, height);
    for y in 0..height {
        for x in 0..width {
            image.set(x, y, colour);
        }
    }
    image
}

#[test]
fn an_unchanged_drawing_is_reported_as_unchanged() {
    let image = filled(20, 10, [10, 20, 30, 255]);
    let difference = compare(&image, &image);
    assert!(difference.is_identical());
    assert_eq!(difference.changed, 0);
    assert!(difference.bands.is_empty());
    assert_eq!(difference.share(), 0.0);
}

/// The case this exists for: a few lines changed, and the report says which rows.
#[test]
fn changed_rows_are_reported_as_the_rows_they_are() {
    let before = filled(20, 10, [10, 20, 30, 255]);
    let mut after = before.clone();
    for x in 0..20 {
        after.set(x, 4, [200, 0, 0, 255]);
        after.set(x, 5, [200, 0, 0, 255]);
        after.set(x, 8, [200, 0, 0, 255]);
    }

    let difference = compare(&before, &after);
    assert!(!difference.is_identical());
    assert_eq!(difference.changed, 60);
    assert_eq!(difference.bands.len(), 2);
    assert_eq!(
        (difference.bands[0].top, difference.bands[0].bottom),
        (4, 5)
    );
    assert_eq!(
        (difference.bands[1].top, difference.bands[1].bottom),
        (8, 8)
    );
    assert_eq!(difference.bands[0].changed, 40);
}

/// A drawing that changed size changed: a line was added, or the letters got taller. Saying so is
/// the point - comparing the overlap and calling it "mostly the same" would hide it.
#[test]
fn a_drawing_that_changed_size_says_so() {
    let before = filled(20, 10, [10, 20, 30, 255]);
    let after = filled(20, 14, [10, 20, 30, 255]);

    let difference = compare(&before, &after);
    assert!(difference.resized);
    assert!(!difference.is_identical());
    assert_eq!(difference.before, (20, 10));
    assert_eq!(difference.after, (20, 14));
    // The overlap itself is unchanged, and the report keeps the two facts apart.
    assert_eq!(difference.changed, 0);
}

/// One pixel is a real difference. A tolerance here would hide exactly the baseline shift that is
/// the reason to look at a picture rather than at a diff of the translations.
#[test]
fn one_pixel_is_a_difference() {
    let before = filled(8, 8, [0, 0, 0, 255]);
    let mut after = before.clone();
    after.set(3, 3, [0, 0, 1, 255]);

    let difference = compare(&before, &after);
    assert_eq!(difference.changed, 1);
    assert_eq!(difference.bands.len(), 1);
}

#[test]
fn the_marked_picture_keeps_what_the_new_drawing_says() {
    let before = filled(6, 6, [0, 0, 0, 0]);
    let mut after = before.clone();
    after.set(2, 2, [255, 255, 255, 255]);

    let picture = marked(&before, &after);
    // Marked, not painted over: the pixel is still opaque where the new drawing drew something.
    assert_eq!(picture.get(2, 2)[3], 255);
    assert_ne!(picture.get(2, 2), before.get(2, 2));
    // And where nothing changed, nothing is marked.
    assert_eq!(picture.get(5, 5), after.get(5, 5));
}

/// Something that used to be drawn and is not any more is invisible in the new picture, so it is
/// marked in a way that can be seen.
#[test]
fn ink_that_disappeared_is_marked() {
    let mut before = filled(6, 6, [0, 0, 0, 0]);
    before.set(1, 1, [255, 255, 255, 255]);
    let after = filled(6, 6, [0, 0, 0, 0]);

    let picture = marked(&before, &after);
    assert_eq!(picture.get(1, 1)[3], 255, "a loss has to be visible");
}

#[test]
fn an_emulator_command_puts_the_game_where_it_was_asked_for() {
    let game = std::path::Path::new("/tmp/out/game.jar");

    let appended = Emulator {
        command: "emulator".into(),
        args: vec!["--fullscreen".into()],
    };
    assert_eq!(
        appended.arguments(game),
        vec!["--fullscreen", "/tmp/out/game.jar"]
    );

    let placed = Emulator {
        command: "emulator".into(),
        args: vec![
            "--jar".into(),
            "{game}".into(),
            "--device".into(),
            "s60".into(),
        ],
    };
    assert_eq!(
        placed.arguments(game),
        vec!["--jar", "/tmp/out/game.jar", "--device", "s60"]
    );
}

// -------------------------------------------------------------------------------------------
// End to end, against a project
// -------------------------------------------------------------------------------------------

use tjlocalizer_core::font::sheet::Grid;
use tjlocalizer_core::jar::Archive;
use tjlocalizer_core::lang::Language;
use tjlocalizer_core::project::{FontProfile, Project};

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tjlocalizer-regress-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A game that draws its own text, so there is something to draw a proof with.
fn game_with_a_font() -> (Vec<u8>, Grid) {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/sample-game.jar"
    ))
    .expect("fixture missing - run tools/make-fixtures.sh");

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
    let mut image = Image::new(columns * cell, rows * cell);
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

    let mut archive = Archive::read(&bytes).unwrap();
    archive.insert("font.png", image.encode_png().unwrap());
    (archive.write().unwrap(), grid)
}

#[test]
fn a_changed_translation_shows_up_in_the_picture() {
    let (bytes, grid) = game_with_a_font();
    let dir = TempDir::new("project");
    let mut project = Project::create(&dir.0, "sample-game", &bytes).unwrap();
    project.profile_mut().font = Some(FontProfile {
        entry: "font.png".into(),
        grid: Some(grid),
        order: String::new(),
        device_font: false,
        mark_library: None,
        marks_from: None,
    });
    project.save().unwrap();

    let graph = project.extract().unwrap();
    let language = project.profile().targets[0].language.clone();
    let node = graph
        .translatable()
        .find(|n| n.source_text == "Quit")
        .unwrap()
        .clone();

    let mut store = project.translations(&language).unwrap();
    store.set(&node.id, "Thoat");
    project.save_translations(&language, &store).unwrap();

    // Nothing accepted yet, so there is nothing to compare against - which is not the same answer
    // as "nothing changed", and is reported as its own thing.
    assert!(project.visual_regression(&language, 1).unwrap().is_none());

    project.accept_baseline(&language, 1).unwrap().unwrap();
    let (same, _) = project.visual_regression(&language, 1).unwrap().unwrap();
    assert!(same.is_identical(), "{same:?}");

    // A translation of the same length: the picture stays the size it was, and the pixels of
    // that one line change. This is the case a size comparison alone would miss.
    let mut store = project.translations(&language).unwrap();
    store.set(&node.id, "Ngung");
    project.save_translations(&language, &store).unwrap();

    let (changed, picture) = project.visual_regression(&language, 1).unwrap().unwrap();
    assert!(!changed.is_identical());
    assert!(!changed.resized, "same length should draw the same size");
    assert!(changed.changed > 0);
    assert_eq!(changed.bands.len(), 1, "one line changed: {changed:?}");
    assert!(picture.exists());

    // And a longer one changes the size of the drawing, which is reported as what it is.
    let mut store = project.translations(&language).unwrap();
    store.set(&node.id, "Thoat khoi tro choi");
    project.save_translations(&language, &store).unwrap();

    let (longer, _) = project.visual_regression(&language, 1).unwrap().unwrap();
    assert!(longer.resized);
    assert!(!longer.is_identical());
}

/// A project that never said what its emulator is does not get one guessed for it.
#[test]
fn playing_without_an_emulator_says_what_is_missing() {
    let (bytes, _) = game_with_a_font();
    let dir = TempDir::new("play");
    let project = Project::create(&dir.0, "sample-game", &bytes).unwrap();
    let language = project.profile().targets[0].language.clone();

    let refused = project.play(&language).unwrap_err().to_string();
    assert!(refused.contains("no emulator"), "{refused}");
}

/// And one that has an emulator but no build says *that*, rather than running the last build of a
/// different language or a stale one.
#[test]
fn playing_before_building_says_there_is_nothing_to_run() {
    let (bytes, _) = game_with_a_font();
    let dir = TempDir::new("play-unbuilt");
    let mut project = Project::create(&dir.0, "sample-game", &bytes).unwrap();
    project.profile_mut().emulator = Some(Emulator {
        command: "definitely-not-a-real-program".into(),
        args: Vec::new(),
    });
    project.save().unwrap();

    let language = Language::new(project.profile().targets[0].language.tag());
    let refused = project.play(&language).unwrap_err().to_string();
    assert!(refused.contains("no build to run"), "{refused}");
}
