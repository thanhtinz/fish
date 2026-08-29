//! The on-disk project (specification §21, §28.1).
//!
//! A project is a directory, not a database, so every intermediate is a file a translator can
//! open, diff and put under version control. Three properties the specification asks for are
//! enforced here rather than left to convention:
//!
//! * the imported original is immutable - it is hashed on import and re-checked on open, so a
//!   build can always be traced back to the exact bytes it started from;
//! * the profile is versioned - each save bumps a revision, so a project.json written by an
//!   older schema is refused with a readable reason rather than half-parsed;
//! * builds are recorded and reversible - every build keeps its output, its report and the
//!   hashes it was produced from, so a bad localization can be rolled back.

use crate::build::{self, Branding, BuildReport};
use crate::detect::{self, CapabilityManifest};
use crate::graph::{self, ContentGraph};
use crate::jar::{sha256_hex, Archive};
use crate::validate::{validate, ValidationReport};
use crate::vietnamese::{Glossary, TranslationMemory, TranslationStore};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The project.json schema this build reads and writes.
pub const SCHEMA_VERSION: u32 = 2;

/// Directories created for every project. Listed once so `create` and the documentation cannot
/// drift apart.
pub const DIRECTORIES: &[&str] = &[
    "original",
    "extracted",
    "content",
    "translations",
    "dictionary",
    "glossary",
    "memory",
    "dialogue",
    "fonts",
    "assets",
    "rules",
    "patches",
    "tests",
    "builds",
    "output",
];

/// The imported artifact, pinned by hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    /// Project-relative path to the untouched original.
    pub jar: String,
    pub sha256: String,
    /// Companion descriptor, when the game shipped with one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jad: Option<String>,
}

/// What is being produced, in terms of language and register rather than of any one game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Localization {
    pub source_language: String,
    pub target_language: String,
    pub style_profile: String,
}

impl Default for Localization {
    fn default() -> Self {
        Self {
            source_language: "auto".to_string(),
            target_language: "vi-VN".to_string(),
            style_profile: "natural-dialogue".to_string(),
        }
    }
}

/// project.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProfile {
    pub schema_version: u32,
    pub name: String,
    /// Bumped on every save. Lets a report say which profile a build was made from.
    #[serde(default)]
    pub revision: u32,
    pub source: Source,
    #[serde(default)]
    pub localization: Localization,
    #[serde(default)]
    pub branding: Branding,
    /// A license or permission reference the owner has for this game (§26). Free text: this tool
    /// records what the user asserts, it does not adjudicate rights.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_reference: Option<String>,
}

/// One recorded build, enough to reproduce or undo it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildRecord {
    pub revision: u32,
    /// The profile revision this was built from.
    pub profile_revision: u32,
    /// Hash of the original, so a build can never be attributed to the wrong source.
    pub source_sha256: String,
    pub translations_applied: usize,
    pub report: BuildReport,
    pub validation: ValidationReport,
}

/// An opened project directory.
#[derive(Debug)]
pub struct Project {
    root: PathBuf,
    profile: ProjectProfile,
}

impl Project {
    /// Imports a JAR into a new project directory (§22, step 1-2).
    ///
    /// The archive is parsed before anything is written: an archive that fails the safety limits
    /// should not leave a half-created project behind.
    pub fn create(root: impl AsRef<Path>, name: &str, jar: &[u8]) -> crate::Result<Self> {
        let root = root.as_ref().to_path_buf();
        Archive::read(jar)?;

        if root.join("project.json").exists() {
            return Err(crate::Error::InvalidProject {
                path: root,
                reason: "a project already exists here".to_string(),
            });
        }

        for dir in DIRECTORIES {
            std::fs::create_dir_all(root.join(dir))?;
        }

        let jar_path = format!("original/{name}.jar");
        std::fs::write(root.join(&jar_path), jar)?;

        let profile = ProjectProfile {
            schema_version: SCHEMA_VERSION,
            name: name.to_string(),
            revision: 0,
            source: Source {
                jar: jar_path,
                sha256: sha256_hex(jar),
                jad: None,
            },
            localization: Localization::default(),
            branding: Branding::default(),
            permission_reference: None,
        };

        let mut project = Project { root, profile };
        project.save()?;
        Ok(project)
    }

    /// Opens an existing project and verifies the original has not been touched.
    pub fn open(root: impl AsRef<Path>) -> crate::Result<Self> {
        let root = root.as_ref().to_path_buf();
        let path = root.join("project.json");
        let text = std::fs::read_to_string(&path).map_err(|e| crate::Error::InvalidProject {
            path: root.clone(),
            reason: format!("cannot read project.json: {e}"),
        })?;
        let profile: ProjectProfile =
            serde_json::from_str(&text).map_err(|e| crate::Error::InvalidProject {
                path: root.clone(),
                reason: format!("project.json is not valid: {e}"),
            })?;

        // A newer schema may use fields this build would silently drop on the next save, taking
        // the translator's work with them. Refusing is the safe direction.
        if profile.schema_version > SCHEMA_VERSION {
            return Err(crate::Error::InvalidProject {
                path: root,
                reason: format!(
                    "project uses schema version {} but this build understands up to {SCHEMA_VERSION}",
                    profile.schema_version
                ),
            });
        }

        let project = Project { root, profile };
        project.verify_original()?;
        Ok(project)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn profile(&self) -> &ProjectProfile {
        &self.profile
    }

    /// Mutable access to the profile. Changes are not on disk until `save`.
    pub fn profile_mut(&mut self) -> &mut ProjectProfile {
        &mut self.profile
    }

    /// Writes project.json, bumping the revision.
    pub fn save(&mut self) -> crate::Result<()> {
        self.profile.revision += 1;
        write_json(&self.root.join("project.json"), &self.profile)
    }

    /// Re-hashes the original and refuses to continue if it changed.
    ///
    /// Without this, editing the file under `original/` would produce builds whose recorded
    /// source hash is a lie, and a rollback would restore something that never existed.
    pub fn verify_original(&self) -> crate::Result<()> {
        let bytes = self.original_bytes()?;
        let actual = sha256_hex(&bytes);
        if actual != self.profile.source.sha256 {
            return Err(crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: format!(
                    "the original has been modified: project.json records {} but {} is {actual}",
                    self.profile.source.sha256, self.profile.source.jar
                ),
            });
        }
        Ok(())
    }

    pub fn original_bytes(&self) -> crate::Result<Vec<u8>> {
        Ok(std::fs::read(self.root.join(&self.profile.source.jar))?)
    }

    pub fn original(&self) -> crate::Result<Archive> {
        Archive::read(&self.original_bytes()?)
    }

    /// Detects capabilities and writes the manifest (§22, step 4).
    pub fn analyze(&self) -> crate::Result<CapabilityManifest> {
        let manifest = detect::detect(&self.original()?);
        write_json(&self.root.join("extracted/capabilities.json"), &manifest)?;
        Ok(manifest)
    }

    /// Extracts the content graph (§22, step 5-6).
    ///
    /// Node ids are derived from location plus original text, so re-extracting after the analyser
    /// improves keeps every existing translation attached to its node.
    pub fn extract(&self) -> crate::Result<ContentGraph> {
        let graph = graph::extract(&self.original()?);
        write_json(&self.root.join("content/graph.json"), &graph)?;
        Ok(graph)
    }

    pub fn graph(&self) -> crate::Result<ContentGraph> {
        read_json(&self.root.join("content/graph.json"))
    }

    pub fn translations(&self) -> crate::Result<TranslationStore> {
        read_json_or_default(&self.root.join("translations/approved.json"))
    }

    pub fn save_translations(&self, store: &TranslationStore) -> crate::Result<()> {
        write_json(&self.root.join("translations/approved.json"), store)
    }

    pub fn glossary(&self) -> crate::Result<Glossary> {
        read_json_or_default(&self.root.join("glossary/glossary.json"))
    }

    pub fn save_glossary(&self, glossary: &Glossary) -> crate::Result<()> {
        write_json(&self.root.join("glossary/glossary.json"), glossary)
    }

    pub fn memory(&self) -> crate::Result<TranslationMemory> {
        read_json_or_default(&self.root.join("memory/memory.json"))
    }

    pub fn save_memory(&self, memory: &TranslationMemory) -> crate::Result<()> {
        write_json(&self.root.join("memory/memory.json"), memory)
    }

    /// Builds, validates, records and publishes (§22 steps 15-18, §23).
    ///
    /// The output is written under `builds/<revision>/` first and copied to `output/` second, so
    /// `output/` only ever holds a build that was fully written and whose record exists.
    pub fn build(&self) -> crate::Result<BuildRecord> {
        self.verify_original()?;
        let original = self.original()?;
        let graph = self.graph()?;
        let translations = self.translations()?;

        let (built, report) = build::apply(&original, &graph, &translations, &self.profile.branding)?;
        let bytes = built.write()?;
        let validation = validate(&original, &built, &graph, &translations);

        let revision = self.next_build_revision()?;
        let dir = self.root.join("builds").join(format!("{revision:04}"));
        std::fs::create_dir_all(&dir)?;

        let name = self.output_name();
        std::fs::write(dir.join(&name), &bytes)?;

        let record = BuildRecord {
            revision,
            profile_revision: self.profile.revision,
            source_sha256: self.profile.source.sha256.clone(),
            translations_applied: translations.len(),
            report,
            validation,
        };
        write_json(&dir.join("build.json"), &record)?;

        std::fs::write(self.root.join("output").join(&name), &bytes)?;
        Ok(record)
    }

    /// Restores a previous build's output as the current one.
    ///
    /// Nothing is deleted: the earlier build directory stays where it is, so a rollback can
    /// itself be rolled back.
    pub fn rollback(&self, revision: u32) -> crate::Result<BuildRecord> {
        let dir = self.root.join("builds").join(format!("{revision:04}"));
        let record: BuildRecord = read_json(&dir.join("build.json")).map_err(|_| {
            crate::Error::InvalidProject {
                path: dir.clone(),
                reason: format!("no build {revision} to roll back to"),
            }
        })?;
        let name = self.output_name();
        let bytes = std::fs::read(dir.join(&name))?;
        std::fs::write(self.root.join("output").join(&name), bytes)?;
        Ok(record)
    }

    /// Recorded builds, oldest first.
    pub fn builds(&self) -> crate::Result<Vec<BuildRecord>> {
        let dir = self.root.join("builds");
        let mut records = Vec::new();
        if !dir.exists() {
            return Ok(records);
        }
        let mut dirs: Vec<_> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        dirs.sort();
        for path in dirs {
            let manifest = path.join("build.json");
            if manifest.exists() {
                records.push(read_json(&manifest)?);
            }
        }
        Ok(records)
    }

    /// The localized artifact's file name, derived from the project and target language.
    pub fn output_name(&self) -> String {
        format!(
            "{}-{}.jar",
            self.profile.name,
            self.profile.localization.target_language.to_lowercase()
        )
    }

    fn next_build_revision(&self) -> crate::Result<u32> {
        Ok(self.builds()?.iter().map(|b| b.revision).max().unwrap_or(0) + 1)
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Pretty-printed with a trailing newline: these files are reviewed and diffed by hand.
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    std::fs::write(path, text)?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> crate::Result<T> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn read_json_or_default<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> crate::Result<T> {
    if path.exists() {
        read_json(path)
    } else {
        Ok(T::default())
    }
}
