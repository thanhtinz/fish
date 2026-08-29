//! A game that sits on disk as a directory rather than in one file.
//!
//! A Steam install is forty thousand files, almost all of them textures, audio and code. Reading
//! it the way an archive is read - every byte into memory - is not possible and would not be
//! useful. So this module is two steps, and keeping them apart is the whole design:
//!
//! * [`scan`] walks the tree and **reads no file's contents**. Paths and sizes only. It is what
//!   makes "41 812 files, 23 of them worth reading" a sentence the interface can say before
//!   anything is opened.
//! * [`ingest`] then reads only the files that pass the allowlist and the size caps, and builds
//!   an [`Archive`] out of them.
//!
//! The second step producing an `Archive` is the point. Twenty-six functions in this crate take
//! `&Archive` - detection, extraction, the build, the rules, validation, the font and asset work -
//! and every one of them runs unchanged on a directory once ingestion has happened. A `Source`
//! enum threaded through eleven signatures would have been the other way, and worse.
//!
//! What is *not* ingested is not forgotten: it is listed, with the reason, because a 300 MB JSON
//! file skipped for being too large is exactly what a translator needs to be told.

use crate::jar::{sha256_hex, Archive};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Extensions worth reading, which is to say the ones something downstream can act on.
///
/// Deliberately the same list `resource::detect` and `writeback::plan` can do something with,
/// rather than "anything that might be text". A wider list means reading gigabytes to find
/// nothing; a narrower one means missing a format this build already handles.
pub const READABLE: &[&str] = &[
    "properties",
    "strings",
    "xml",
    "json",
    "po",
    "pot",
    "ini",
    "cfg",
    "txt",
    "rpy",
    "rpym",
    "locres",
    "csv",
    "lang",
    "loc",
];

/// What a scan will read, and what it will refuse to.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Per file. A localization file is kilobytes; a megabyte-scale one is a data table.
    pub max_file_size: u64,
    /// Across everything ingested, because the result is held in memory.
    pub max_total_size: u64,
    /// How many files may be ingested.
    pub max_files: usize,
    /// How deep to walk. Game trees are wide, not deep.
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_size: 8 * 1024 * 1024,
            max_total_size: 64 * 1024 * 1024,
            max_files: 5_000,
            max_depth: 12,
        }
    }
}

/// One file the scan found, before anything was read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    /// Relative to the game root, `/`-separated, so it can be an archive entry name unchanged.
    pub path: String,
    pub size: u64,
}

/// Everything the tree holds, established without opening a single file.
#[derive(Debug, Clone, Default)]
pub struct Scan {
    pub files: Vec<Found>,
    /// The total size of everything, ingested or not. What a person means by "how big is it".
    pub total_size: u64,
    /// What this looks like it was made with, from file names alone.
    pub evidence: Vec<String>,
}

/// Why a file the scan found was not read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub path: String,
    pub size: u64,
    /// In words, for a person. "too large" and "not a format this build reads" are different
    /// facts, and rolling them together would hide the first behind the second.
    pub reason: String,
}

/// The result of reading the files worth reading.
#[derive(Debug)]
pub struct Ingested {
    pub archive: Archive,
    /// Every file that was read, with its hash. This is what gets pinned and copied.
    pub files: Vec<Pinned>,
    /// Files passed over, and why. Only the ones a person might have expected: a texture is not
    /// listed, a 300 MB JSON is.
    pub skipped: Vec<Skipped>,
    /// How many files the tree holds in total, read or not.
    pub scanned: usize,
    pub total_size: u64,
    pub evidence: Vec<String>,
}

/// One file that was read, pinned by content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pinned {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// Walks the tree without opening anything.
///
/// Errors from individual entries are swallowed rather than failing the walk, the same way the
/// font library scan does it: a game folder has files the user cannot read, and one of them should
/// not stop the other forty thousand being listed.
pub fn scan(root: &Path, limits: &Limits) -> Scan {
    let mut files = Vec::new();
    let mut total_size = 0u64;

    for entry in walkdir::WalkDir::new(root)
        .max_depth(limits.max_depth)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(root) else {
            continue;
        };
        let path = relative.to_string_lossy().replace('\\', "/");
        // A name that would not survive being an archive entry is a name this pipeline cannot
        // address, and silently renaming somebody's file is worse than skipping it.
        if path.is_empty() || path.starts_with('/') || path.split('/').any(|p| p == "..") {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        total_size += size;
        files.push(Found { path, size });
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    let evidence = engine_evidence(&files);
    Scan {
        files,
        total_size,
        evidence,
    }
}

/// What the game looks like it was made with, from names alone.
///
/// Free, because the scan has already listed every name, and useful out of proportion to its cost:
/// it is the difference between "this is a folder" and "this is an Unreal game, so look for
/// `.locres`". It is evidence and reads as evidence - the caller shows it beside what it concluded
/// so a wrong answer can be argued with.
pub fn engine_evidence(files: &[Found]) -> Vec<String> {
    let mut evidence = Vec::new();
    let any = |f: &dyn Fn(&str) -> bool| files.iter().any(|e| f(&e.path));

    if any(&|p| p.ends_with("/globalgamemanagers") || p.ends_with("_Data/resources.assets")) {
        evidence.push("a Unity *_Data directory".into());
    }
    if any(&|p| p == "project.godot" || p.ends_with(".pck")) {
        evidence.push("a Godot project or package".into());
    }
    if any(&|p| p.starts_with("game/") && p.ends_with(".rpy")) {
        evidence.push("a Ren'Py game/ directory".into());
    }
    if any(&|p| p.contains("Engine/Binaries/") || p.ends_with(".locres")) {
        evidence.push("Unreal Engine binaries or a compiled string table".into());
    }
    if any(&|p| {
        let name = p.rsplit('/').next().unwrap_or(p);
        name.starts_with("steam_api")
    }) {
        evidence.push("the Steam API library".into());
    }
    if evidence.is_empty() {
        evidence.push("no engine this build recognises by name".into());
    }
    evidence
}

/// Reads the files worth reading and builds an archive from them.
///
/// Everything that follows in this crate takes an `&Archive`, so this is the one place a directory
/// stops being a directory.
pub fn ingest(root: &Path, scan: Scan, limits: &Limits) -> crate::Result<Ingested> {
    let mut archive = Archive::empty();
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    let mut total = 0u64;
    let scanned = scan.files.len();

    for found in &scan.files {
        if !worth_reading(&found.path) {
            // Not listed. A game folder is mostly textures and audio, and naming every one of them
            // as "skipped" would bury the two files a person actually needs to see.
            continue;
        }
        if found.size > limits.max_file_size {
            skipped.push(Skipped {
                path: found.path.clone(),
                size: found.size,
                reason: format!(
                    "larger than the {} MiB this build reads in one file",
                    limits.max_file_size / (1024 * 1024)
                ),
            });
            continue;
        }
        if files.len() >= limits.max_files {
            skipped.push(Skipped {
                path: found.path.clone(),
                size: found.size,
                reason: format!("past the {} file limit for one project", limits.max_files),
            });
            continue;
        }
        if total + found.size > limits.max_total_size {
            skipped.push(Skipped {
                path: found.path.clone(),
                size: found.size,
                reason: format!(
                    "past the {} MiB this build holds in memory at once",
                    limits.max_total_size / (1024 * 1024)
                ),
            });
            continue;
        }

        let Ok(data) = std::fs::read(root.join(&found.path)) else {
            skipped.push(Skipped {
                path: found.path.clone(),
                size: found.size,
                reason: "could not be read".into(),
            });
            continue;
        };
        total += data.len() as u64;
        files.push(Pinned {
            path: found.path.clone(),
            size: data.len() as u64,
            sha256: sha256_hex(&data),
        });
        archive.insert(found.path.clone(), data);
    }

    Ok(Ingested {
        archive,
        files,
        skipped,
        scanned,
        total_size: scan.total_size,
        evidence: scan.evidence,
    })
}

fn worth_reading(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path).to_lowercase();
    match name.rsplit_once('.') {
        Some((_, extension)) => READABLE.contains(&extension),
        None => false,
    }
}

/// The hash of a whole tree, as one string.
///
/// A directory has no bytes of its own to hash, so this hashes a manifest of what is in it: every
/// ingested path and its hash, sorted, joined. It is deterministic and it means `verify_original`
/// stays a single comparison rather than becoming a loop with its own failure modes.
pub fn manifest_sha256(files: &[Pinned]) -> String {
    let mut lines: Vec<String> = files
        .iter()
        .map(|f| format!("{}\0{}", f.path, f.sha256))
        .collect();
    lines.sort();
    sha256_hex(lines.join("\n").as_bytes())
}

/// What was pinned when the project was created, kept beside the copies.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeRecord {
    /// Where the game was when it was imported. Recorded so a person can find it again; never
    /// trusted, because a game moves and a drive gets remounted.
    pub root: String,
    /// Every file that was read, hashed and copied under `original/tree/`.
    pub files: Vec<Pinned>,
    /// Every file that was passed over for a reason worth saying.
    #[serde(default)]
    pub skipped: Vec<Skipped>,
    /// How many files the game holds in total.
    pub scanned: usize,
    pub total_size: u64,
    /// Said out loud rather than left to be inferred: the files that were not read were not
    /// hashed either, so nothing here can tell whether one of them changed. Hashing forty
    /// gigabytes on every open is not a thing anyone would wait for.
    pub unread_files_are_not_hashed: bool,
    #[serde(default)]
    pub evidence: Vec<String>,
}
