//! A folder of fonts, and picking the right one for a sheet.
//!
//! Anyone who localizes into Vietnamese has a folder of fonts. Making the tool read that folder is
//! more useful than shipping one font, and it puts nobody's typeface inside this repository - the
//! files stay where their owner keeps them.
//!
//! Picking is done by measurement rather than by name. Which font gives the best diacritics for a
//! given sheet depends on the cell size and on the game's own letterforms, and the only way to
//! know is to compose the letters with each candidate and count how many marks survive the rule
//! that keeps letters distinguishable. A font that looks right at reading size can contribute
//! almost nothing at twelve pixels.

use crate::font::outline::MarkSource;
use crate::font::sheet::{extend_with_marks, Sheet};
use crate::font::vietnamese_compositions;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One font found on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub path: PathBuf,
    pub name: String,
    /// Whether it can draw all 134 Vietnamese letters. A font that cannot is not useless - it may
    /// still cover the common ones - but it cannot supply every mark.
    pub covers_vietnamese: bool,
    /// How many of the 134 it draws.
    pub covered: usize,
}

/// Every font in a directory, deepest first, that this build can read.
///
/// Unreadable files are skipped rather than failing the scan: a font folder collected over years
/// has broken files in it, and one of them should not stop the other five hundred being usable.
pub fn scan(directory: &Path) -> Result<Vec<Candidate>> {
    let mut found = Vec::new();
    let required = crate::font::vietnamese_required();

    for entry in walkdir::WalkDir::new(directory)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !matches!(extension.as_str(), "ttf" | "otf" | "ttc" | "otc") {
            continue;
        }
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let Ok(source) = MarkSource::load(bytes, path.display().to_string()) else {
            continue;
        };
        let covered = required.iter().filter(|c| source.has(**c)).count();

        found.push(Candidate {
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            covers_vietnamese: covered == required.len(),
            covered,
            path: path.to_path_buf(),
        });
    }

    found.sort_by(|a, b| b.covered.cmp(&a.covered).then(a.name.cmp(&b.name)));
    Ok(found)
}

/// How well one font actually serves one sheet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fit {
    pub path: PathBuf,
    pub name: String,
    /// Marks this font supplied, out of the letters composed.
    pub from_typeface: usize,
    pub composed: usize,
}

impl Fit {
    pub fn share(&self) -> f32 {
        if self.composed == 0 {
            0.0
        } else {
            self.from_typeface as f32 / self.composed as f32
        }
    }
}

/// Tries each candidate against a sheet and ranks them by how many marks they actually supply.
///
/// This is the honest way to choose. At twelve pixels a well-drawn typeface may contribute a third
/// of its marks and a plainer one two thirds, and no property of the file says which.
pub fn rank(sheet: &Sheet, candidates: &[Candidate]) -> Result<Vec<Fit>> {
    let compositions = vietnamese_compositions();
    let mut fits = Vec::new();

    for candidate in candidates {
        let Ok(bytes) = std::fs::read(&candidate.path) else {
            continue;
        };
        let Ok(source) = MarkSource::load(bytes, candidate.path.display().to_string()) else {
            continue;
        };
        let (_, report) = extend_with_marks(sheet, &compositions, Some(&source))?;
        fits.push(Fit {
            path: candidate.path.clone(),
            name: candidate.name.clone(),
            from_typeface: report.from_typeface,
            composed: report.added.len(),
        });
    }

    fits.sort_by(|a, b| {
        b.from_typeface
            .cmp(&a.from_typeface)
            .then(a.name.cmp(&b.name))
    });
    Ok(fits)
}

/// The best font in a folder for one sheet, or `None` when none contributes anything.
pub fn best_for(sheet: &Sheet, directory: &Path) -> Result<Option<Fit>> {
    // Only fonts covering the whole alphabet are tried: a partial one produces a sheet whose marks
    // come from two sources and do not match each other, which looks worse than one source.
    let candidates: Vec<Candidate> = scan(directory)?
        .into_iter()
        .filter(|c| c.covers_vietnamese)
        .collect();
    Ok(rank(sheet, &candidates)?
        .into_iter()
        .find(|f| f.from_typeface > 0))
}
