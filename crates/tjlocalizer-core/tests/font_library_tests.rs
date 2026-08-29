//! A folder of fonts: finding them, and choosing between them by measurement.

use tjlocalizer_core::font::library;
use tjlocalizer_core::font::sheet::{preview, render_line, Grid, Image, Sheet};

/// A directory of system fonts, if this machine has one.
fn font_directory() -> Option<std::path::PathBuf> {
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/truetype/dejavu",
        "/usr/share/fonts/truetype",
        "/Library/Fonts",
        "C:/Windows/Fonts",
    ];
    CANDIDATES
        .iter()
        .map(std::path::PathBuf::from)
        .find(|p| p.is_dir())
}

fn sheet(cell: u32, ink: u32, padding: u32) -> Sheet {
    let characters: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    let columns = 16u32;
    let rows = (characters.len() as u32).div_ceil(columns);
    let mut image = Image::new(columns * cell, rows * cell);
    let grid = Grid {
        cell_width: cell,
        cell_height: cell,
        columns,
        rows,
    };
    for (i, c) in characters.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let (ox, oy) = grid.cell_origin(i as u32);
        let seed = (*c as u32).wrapping_mul(2_654_435_761);
        let width = ink.min(6);
        for y in 0..ink {
            for x in 0..width {
                let edge = x == 0 || x + 1 == width || y == 0 || y + 1 == ink;
                if edge || (seed >> ((y * 4 + x) % 24)) & 1 == 1 {
                    image.set(ox + 2 + x, oy + padding + y, [255, 255, 255, 255]);
                }
            }
        }
    }
    Sheet::ascii(image, grid)
}

#[test]
fn a_folder_of_fonts_is_read_and_its_vietnamese_coverage_counted() {
    let Some(directory) = font_directory() else {
        eprintln!("no system font directory; skipping");
        return;
    };
    let found = library::scan(&directory).unwrap();
    assert!(
        !found.is_empty(),
        "{} held no readable font",
        directory.display()
    );

    for candidate in &found {
        assert!(candidate.covered <= 134);
        assert_eq!(candidate.covers_vietnamese, candidate.covered == 134);
        assert!(!candidate.name.is_empty());
    }
    // Sorted by coverage, so the most useful font is first.
    assert!(found.windows(2).all(|w| w[0].covered >= w[1].covered));
}

/// A font folder collected over years has broken files in it, and one of them must not stop the
/// other five hundred from being usable.
#[test]
fn unreadable_files_are_skipped_rather_than_failing_the_scan() {
    let directory = std::env::temp_dir().join(format!(
        "tjlocalizer-fonts-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("broken.ttf"), b"this is not a font").unwrap();
    std::fs::write(directory.join("notes.txt"), b"nor is this").unwrap();

    let found = library::scan(&directory).unwrap();
    assert!(found.is_empty());
    std::fs::remove_dir_all(&directory).ok();
}

#[test]
fn an_empty_folder_yields_nothing_rather_than_an_error() {
    let directory = std::env::temp_dir().join("tjlocalizer-empty-fonts");
    std::fs::create_dir_all(&directory).unwrap();
    assert!(library::scan(&directory).unwrap().is_empty());
    std::fs::remove_dir_all(&directory).ok();
}

/// Which font serves a sheet best is not a property of the file: it depends on the cell size, and
/// the only way to know is to compose the letters and count what survives.
#[test]
fn fonts_are_ranked_against_the_sheet_they_will_be_used_with() {
    let Some(directory) = font_directory() else {
        return;
    };
    let candidates: Vec<_> = library::scan(&directory)
        .unwrap()
        .into_iter()
        .filter(|c| c.covers_vietnamese)
        .take(4)
        .collect();
    if candidates.is_empty() {
        eprintln!("no system font covers Vietnamese; skipping");
        return;
    }

    let fits = library::rank(&sheet(24, 11, 11), &candidates).unwrap();
    assert_eq!(fits.len(), candidates.len());
    // Best first.
    assert!(fits
        .windows(2)
        .all(|w| w[0].from_typeface >= w[1].from_typeface));
    for fit in &fits {
        assert_eq!(fit.composed, 134);
        assert!(fit.from_typeface <= fit.composed);
        assert!((0.0..=1.0).contains(&fit.share()));
    }
}

/// The same font serves a larger cell better, because its diacritics stop thinning out.
#[test]
fn the_ranking_changes_with_the_cell_size() {
    let Some(directory) = font_directory() else {
        return;
    };
    let candidates: Vec<_> = library::scan(&directory)
        .unwrap()
        .into_iter()
        .filter(|c| c.covers_vietnamese)
        .take(2)
        .collect();
    if candidates.is_empty() {
        return;
    }

    let small = library::rank(&sheet(12, 5, 5), &candidates).unwrap();
    let large = library::rank(&sheet(32, 15, 14), &candidates).unwrap();
    let total = |fits: &[library::Fit]| fits.iter().map(|f| f.from_typeface).sum::<usize>();

    assert!(
        total(&large) > total(&small),
        "a bigger cell should take more marks: {} against {}",
        total(&large),
        total(&small)
    );
}

#[test]
fn the_best_font_for_a_sheet_is_one_that_supplies_marks() {
    let Some(directory) = font_directory() else {
        return;
    };
    match library::best_for(&sheet(24, 11, 11), &directory).unwrap() {
        Some(fit) => assert!(fit.from_typeface > 0),
        None => eprintln!("no font here supplies a mark at 24px; that is a valid answer"),
    }
}

#[test]
fn a_line_is_drawn_from_the_sheet_cell_by_cell() {
    let base = sheet(12, 5, 4);
    let line = render_line(&base, "AB");
    assert_eq!(line.width, 24);
    assert_eq!(line.height, 12);

    // The first cell must be A's, pixel for pixel.
    let index = base.index_of('A').unwrap();
    let (ox, oy) = base.grid.cell_origin(index);
    for y in 0..12 {
        for x in 0..12 {
            assert_eq!(line.get(x, y), base.image.get(ox + x, oy + y));
        }
    }
}

/// A character the sheet cannot draw leaves a gap, because that is what the game shows - not a
/// substitute, which would hide the problem the font report exists to surface.
#[test]
fn a_character_the_sheet_lacks_is_left_blank() {
    let base = sheet(12, 5, 4);
    let line = render_line(&base, "ế");
    assert!(line.pixels.iter().all(|b| *b == 0));
}

#[test]
fn a_preview_stacks_every_line_at_both_sizes() {
    let base = sheet(12, 5, 4);
    let image = preview(&[("drawn", &base)], &["AB", "CD"], 4);

    // Two lines, each at 1x and 4x, plus a gap after each.
    let expected = (12 + 4) * 2 + (48 + 4) * 2;
    assert_eq!(image.height, expected);
    assert_eq!(image.width, 24 * 4);
}
