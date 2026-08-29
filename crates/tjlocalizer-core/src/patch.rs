//! A patch: the files a build changed, and nothing else.
//!
//! A game that came in as one file goes out as one file. A game that came in as a directory
//! cannot: copying forty thousand files to change three of them is not a build, it is a second
//! copy of somebody's game sitting in their project folder. So a directory build produces a patch
//! - the changed files at their own relative paths, plus a manifest.
//!
//! The manifest is what makes applying one safe. Every file carries the hash it had *before* the
//! change as well as after, so applying to a game that is not the one the patch was built from
//! fails on the first file rather than half-writing a game nobody can put back.
//!
//! Applying is deliberately a separate, explicit act. `build` never writes into a game directory;
//! that is the most destructive thing this tool can do and it does not happen as a side effect of
//! anything.

use crate::jar::{sha256_hex, Archive};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One file a build changed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    /// Relative to the game root, `/`-separated.
    pub path: String,
    /// What the file hashed to before. Checked against the real game before anything is written:
    /// this is what lets a patch refuse a game it was not built from.
    pub before: String,
    pub after: String,
    pub size: u64,
}

/// The manifest written beside a patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub project: String,
    pub language: String,
    pub revision: u32,
    pub changes: Vec<Change>,
    /// Who made the localization. Attribution travels with the patch rather than being written
    /// into the game: a stray file in an install directory is something a game's own integrity
    /// check may notice, and the patch is what gets sent to another person anyway.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub localized_by: Option<String>,
}

/// Works out what changed, and writes the patch.
///
/// Compares the built archive against the pinned originals rather than against the game on disk:
/// what the build changed is a fact about the build, and reading the game here would make it a
/// fact about whatever the game happens to be right now.
pub fn write(
    directory: &Path,
    original: &Archive,
    built: &Archive,
    manifest: &mut Manifest,
) -> crate::Result<()> {
    std::fs::create_dir_all(directory)?;
    manifest.changes.clear();

    for entry in built.entries() {
        let before = match original.get(&entry.name) {
            Some(was) => was,
            // A file the build added rather than changed. Nothing does this for a directory game
            // today - branding is deliberately not written into the game - and adding a file to
            // somebody's install is a different act from changing one, so it is left out rather
            // than quietly included.
            None => continue,
        };
        if before.data == entry.data {
            continue;
        }

        let destination = directory.join(&entry.name);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&destination, &entry.data)?;

        manifest.changes.push(Change {
            path: entry.name.clone(),
            before: sha256_hex(&before.data),
            after: sha256_hex(&entry.data),
            size: entry.data.len() as u64,
        });
    }

    manifest.changes.sort_by(|a, b| a.path.cmp(&b.path));
    let json = serde_json::to_string_pretty(manifest)?;
    std::fs::write(directory.join("patch.json"), json)?;
    std::fs::write(directory.join("INSTALL.txt"), instructions(manifest))?;
    Ok(())
}

/// Reads a patch back.
pub fn read(directory: &Path) -> crate::Result<Manifest> {
    let text = std::fs::read_to_string(directory.join("patch.json")).map_err(|e| {
        crate::Error::InvalidProject {
            path: directory.to_path_buf(),
            reason: format!("no patch here: {e}"),
        }
    })?;
    serde_json::from_str(&text).map_err(|e| crate::Error::InvalidProject {
        path: directory.to_path_buf(),
        reason: format!("the patch manifest is not valid: {e}"),
    })
}

/// What applying this patch would do, checked against a real game directory.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Files that match what the patch expects and would be overwritten.
    pub ready: Vec<Change>,
    /// Files that do not, and why. A non-empty list means the patch is refused entirely.
    pub mismatched: Vec<Mismatch>,
}

#[derive(Debug, Clone)]
pub struct Mismatch {
    pub path: String,
    pub reason: String,
}

impl Plan {
    pub fn is_applicable(&self) -> bool {
        self.mismatched.is_empty() && !self.ready.is_empty()
    }
}

/// Checks a patch against a game without writing anything.
///
/// Every file is checked before any is written, so a patch that cannot be applied cleanly is not
/// applied at all. Half a translation is worse than none: the game is then in a state neither the
/// patch nor a backup describes.
pub fn plan(manifest: &Manifest, game: &Path) -> Plan {
    let mut ready = Vec::new();
    let mut mismatched = Vec::new();

    for change in &manifest.changes {
        let path = game.join(&change.path);
        match std::fs::read(&path) {
            Ok(data) => {
                let actual = sha256_hex(&data);
                if actual == change.before {
                    ready.push(change.clone());
                } else if actual == change.after {
                    mismatched.push(Mismatch {
                        path: change.path.clone(),
                        reason: "already holds this patch".into(),
                    });
                } else {
                    mismatched.push(Mismatch {
                        path: change.path.clone(),
                        reason: "is not the version this patch was built from".into(),
                    });
                }
            }
            Err(e) => mismatched.push(Mismatch {
                path: change.path.clone(),
                reason: format!("could not be read: {e}"),
            }),
        }
    }

    Plan { ready, mismatched }
}

/// Applies a patch, keeping what it overwrote.
///
/// Three phases, in this order, and the order is the contract rather than a suggestion: check
/// everything, back up everything, then write. An apply that fails partway has to leave the game
/// exactly as it was.
pub fn apply(
    manifest: &Manifest,
    game: &Path,
    patch: &Path,
    backup: &Path,
) -> crate::Result<Vec<String>> {
    let plan = plan(manifest, game);
    if !plan.mismatched.is_empty() {
        let named: Vec<String> = plan
            .mismatched
            .iter()
            .map(|m| format!("{} {}", m.path, m.reason))
            .collect();
        return Err(crate::Error::InvalidProject {
            path: game.to_path_buf(),
            reason: format!(
                "this patch was not built from this copy of the game, so none of it was \
                 applied:\n  {}",
                named.join("\n  ")
            ),
        });
    }
    if plan.ready.is_empty() {
        return Ok(Vec::new());
    }

    // Every backup taken before the first write, so a failure halfway leaves a complete record of
    // what the game held.
    for change in &plan.ready {
        let destination = backup.join(&change.path);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(game.join(&change.path), destination)?;
    }

    let mut written = Vec::new();
    for change in &plan.ready {
        std::fs::copy(patch.join(&change.path), game.join(&change.path))?;
        written.push(change.path.clone());
    }
    Ok(written)
}

fn instructions(manifest: &Manifest) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{} - {} localization\n\n",
        manifest.project, manifest.language
    ));
    out.push_str(
        "This folder holds only the files that changed. Copy them over the same paths in your\n\
         game directory, keeping the folder structure, or let the tool do it:\n\n    \
         tjlocalizer apply-patch <project> --to <game directory>\n\n\
         Applying through the tool checks that each file is the version this patch was built\n\
         from, and keeps a copy of what it replaced.\n\n",
    );
    out.push_str(&format!("Files changed ({}):\n", manifest.changes.len()));
    for change in &manifest.changes {
        out.push_str(&format!("  {}\n", change.path));
    }
    if let Some(author) = &manifest.localized_by {
        out.push_str(&format!(
            "\nLocalization by {author}. This covers the translation only; the game and\n\
             everything in it belong to their original authors.\n"
        ));
    }
    out
}
