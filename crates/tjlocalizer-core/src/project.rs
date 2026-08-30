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
use crate::dictionary::Dictionary;
use crate::font::outline::MarkSource;
use crate::font::sheet::{extend_with_marks, Grid, Image, Sheet};
use crate::font::{self, Coverage, CoverageReport};
use crate::graph::{self, ContentGraph};
use crate::jar::{sha256_hex, Archive};
use crate::lang::Language;
use crate::provider::ProviderConfig;
use crate::suggest::{self, CandidateSet};
use crate::translation::{Glossary, TranslationMemory, TranslationStore};
use crate::validate::ValidationReport;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The project.json schema this build reads and writes.
///
/// Version 3 replaced the single `localization` object with a list of targets, so one project can
/// be shipped in several languages from one body of extracted text. Version 4 made the source a
/// tagged union, because a game can now be a directory rather than a file. Version 5 added the
/// emulator a person runs their builds in and how each image's words were established. Older
/// projects are migrated on open rather than refused.
pub const SCHEMA_VERSION: u32 = 5;

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

/// Where the game came from, pinned by hash.
///
/// Tagged rather than untagged: `#[serde(untagged)]` would pick whichever variant happened to
/// deserialize, and a project file matched to the wrong variant is the kind of bug that eats
/// somebody's work quietly. The tag makes a wrong file an error instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Source {
    /// One file - a JAR, an APK, an IPA, a zip - copied whole into the project.
    Archive {
        /// Project-relative path to the untouched original.
        jar: String,
        sha256: String,
        /// Companion descriptor, when the game shipped with one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        jad: Option<String>,
    },
    /// A game that sits on disk as a directory, of which the files worth reading were copied in.
    Tree {
        /// Where the game was when it was imported. Recorded so a person can find it again, never
        /// trusted: a game moves, and a drive gets remounted somewhere else.
        root: String,
        /// The hash of a manifest of everything ingested - see `tree::manifest_sha256`. A
        /// directory has no bytes of its own to hash.
        sha256: String,
    },
}

impl Source {
    /// The hash that pins this source, whichever kind it is.
    pub fn sha256(&self) -> &str {
        match self {
            Source::Archive { sha256, .. } | Source::Tree { sha256, .. } => sha256,
        }
    }

    /// What to call this source in a message to a person.
    pub fn label(&self) -> &str {
        match self {
            Source::Archive { jar, .. } => jar,
            Source::Tree { root, .. } => root,
        }
    }

    pub fn is_tree(&self) -> bool {
        matches!(self, Source::Tree { .. })
    }
}

/// One language this project is being shipped in.
///
/// A project has a list of these rather than one. The extracted text, the glossary decisions
/// about the *source* and the capability manifest are shared; the approved translations, the
/// register and the builds are per target, because they are separate bodies of work reviewed
/// separately.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub language: Language,
    /// The register to write in, by id from `register::builtin_profiles`.
    pub style_profile: String,
    /// Set aside without deleting its translations, so a language can be paused.
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

impl Target {
    pub fn new(language: Language, style_profile: impl Into<String>) -> Self {
        Self {
            language,
            style_profile: style_profile.into(),
            enabled: true,
        }
    }

    /// A file-name-safe form of the language tag: `vi-VN` gives `vi-vn`.
    pub fn slug(&self) -> String {
        self.language
            .tag()
            .to_lowercase()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "-")
    }
}

/// The language the game is written in, and how that was decided.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLanguage {
    pub language: Language,
    /// True when detection chose it rather than a person. Kept because a wrong source language
    /// silently disables every dictionary, and a translator needs to see that it was a guess.
    #[serde(default)]
    pub detected: bool,
}

impl Default for SourceLanguage {
    fn default() -> Self {
        Self {
            language: Language::new("und"),
            detected: false,
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
    pub source_language: SourceLanguage,
    /// The languages being produced. Never empty: a project with no target cannot be built, and
    /// an empty list is more likely a migration bug than an intention.
    #[serde(default)]
    pub targets: Vec<Target>,
    #[serde(default)]
    pub branding: Branding,
    /// A license or permission reference the owner has for this game (§26). Free text: this tool
    /// records what the user asserts, it does not adjudicate rights.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_reference: Option<String>,
    /// An external translation engine, if the user configured one. Off by default, and the key is
    /// deliberately not here: this file is committed and sent to translators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderConfig>,
    /// How the analysis side is set up, if the user turned it on. Off by default, and like the
    /// engine above it holds no key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude: Option<crate::claude::Settings>,
    /// Images somebody has decided carry words painted into them (§17).
    ///
    /// Empty means nobody has looked, which is not the same as "there are none" - and the
    /// difference matters, because artwork with English on it survives a translation that every
    /// check in this project passes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_assets: Vec<crate::assets::TextAsset>,
    /// The game's glyph sheet, when it draws its own text (§16).
    ///
    /// Absent means "not established". It does not mean the game uses the device font: a game
    /// that draws from a sheet and has no profile here will show blanks, and saying nothing about
    /// that would be the worst of the three answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<FontProfile>,
    /// The emulator this project's owner runs their builds in (§25).
    ///
    /// Their choice, written down so it does not have to be retyped. Absent means they have not
    /// said, and nothing here guesses: no emulator is shipped, suggested or downloaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emulator: Option<crate::regress::Emulator>,
}

/// Something in a game's code that looks like part of its font lookup (§16).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontLookup {
    pub class: String,
    pub what: LookupEvidence,
    /// What was found there, as text: a number, or the string listing the characters.
    pub value: String,
}

/// What kind of thing was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LookupEvidence {
    /// A number equal to the sheet's column count.
    Columns,
    /// A number equal to its row count - the one a taller sheet changes.
    Rows,
    CellWidth,
    CellHeight,
    /// A string listing the sheet's characters, in the sheet's order.
    Order,
}

impl LookupEvidence {
    /// What this is, for an interface that has to say it.
    pub fn key(self) -> &'static str {
        match self {
            LookupEvidence::Columns => "columns",
            LookupEvidence::Rows => "rows",
            LookupEvidence::CellWidth => "cell-width",
            LookupEvidence::CellHeight => "cell-height",
            LookupEvidence::Order => "character-order",
        }
    }
}

/// Where a game's glyph sheet is and how it is laid out.
///
/// Given rather than guessed at: a grid inferred from one sheet is a guess about that sheet, and
/// a wrong guess shifts every glyph by a pixel in a way that looks like a rendering bug.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontProfile {
    /// Archive entry holding the sheet, or empty when the game uses the device font.
    pub entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid: Option<Grid>,
    /// The characters the sheet lays out, in order. Empty means printable ASCII, which is what
    /// most game sheets are.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub order: String,
    /// True when the game uses the device font and can draw whatever the handset can.
    #[serde(default)]
    pub device_font: bool,
    /// A folder of fonts to take diacritic shapes from.
    ///
    /// A path, not the fonts. They stay where their owner keeps them; nothing is copied into the
    /// project, and the project can be shared without carrying somebody's typefaces along.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_library: Option<PathBuf>,
    /// The font chosen from that folder, once one has been measured against this sheet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marks_from: Option<PathBuf>,
}

/// One recorded build of one language, enough to reproduce or undo it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildRecord {
    pub revision: u32,
    /// Which target this build was for.
    #[serde(default)]
    pub language: Language,
    /// The profile revision this was built from.
    pub profile_revision: u32,
    /// Hash of the original, so a build can never be attributed to the wrong source.
    pub source_sha256: String,
    pub translations_applied: usize,
    pub report: BuildReport,
    /// The per-game patches that ran, if any (§19).
    #[serde(default)]
    pub rules: crate::rules::Applied,
    pub validation: ValidationReport,
}

/// The extension an imported package should keep on disk.
///
/// An Android package stored as `.jar` is still an Android package, but nothing that opens it
/// knows that, starting with the person who goes looking for it in a file manager.
fn extension_of(archive: &Archive) -> &'static str {
    match crate::package::detect(archive).kind {
        crate::package::Kind::Apk => "apk",
        crate::package::Kind::Ipa => "ipa",
        crate::package::Kind::Zip => "zip",
        _ => "jar",
    }
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
        let archive = Archive::read(jar)?;

        if root.join("project.json").exists() {
            return Err(crate::Error::InvalidProject {
                path: root,
                reason: "a project already exists here".to_string(),
            });
        }

        for dir in DIRECTORIES {
            std::fs::create_dir_all(root.join(dir))?;
        }

        let jar_path = format!("original/{name}.{}", extension_of(&archive));
        std::fs::write(root.join(&jar_path), jar)?;

        // Guessing the source language from the archive beats defaulting to English: a wrong
        // source language silently disables every dictionary, and the guess is recorded as a
        // guess so a person can see it was never confirmed.
        let detected = crate::detect::detect_source_language(&archive);

        let profile = ProjectProfile {
            schema_version: SCHEMA_VERSION,
            name: name.to_string(),
            revision: 0,
            source: Source::Archive {
                jar: jar_path,
                sha256: sha256_hex(jar),
                jad: None,
            },
            source_language: SourceLanguage {
                language: detected.0,
                detected: true,
            },
            targets: vec![Target::new(Language::new("vi-VN"), "natural-dialogue")],
            branding: Branding::default(),
            permission_reference: None,
            provider: None,
            claude: None,
            text_assets: Vec::new(),
            font: None,
            emulator: None,
        };

        let mut project = Project { root, profile };
        project.save()?;
        Ok(project)
    }

    /// Imports a game that sits on disk as a directory.
    ///
    /// A sibling of `create` rather than a wider `create`, because `create`'s signature is used by
    /// nineteen tests that have nothing to do with directories, and widening it would mean editing
    /// all of them to say the same thing they say now.
    ///
    /// Only the files worth reading are copied in - see `tree::ingest`. They are *copied*, not
    /// only hashed, because the original has to stay reachable byte for byte after Steam has
    /// updated over the game.
    pub fn create_from_tree(
        root: impl AsRef<Path>,
        name: &str,
        game: impl AsRef<Path>,
        limits: &crate::tree::Limits,
    ) -> crate::Result<(Self, crate::tree::Ingested)> {
        let root = root.as_ref().to_path_buf();
        let game = game.as_ref();

        if root.join("project.json").exists() {
            return Err(crate::Error::InvalidProject {
                path: root,
                reason: "a project already exists here".to_string(),
            });
        }
        if !game.is_dir() {
            return Err(crate::Error::InvalidProject {
                path: game.to_path_buf(),
                reason: "not a directory".to_string(),
            });
        }

        let scan = crate::tree::scan(game, limits);
        let ingested = crate::tree::ingest(game, scan, limits)?;
        if ingested.files.is_empty() {
            return Err(crate::Error::InvalidProject {
                path: game.to_path_buf(),
                reason: format!(
                    "none of the {} files here are in a format this build can read",
                    ingested.scanned
                ),
            });
        }

        for dir in DIRECTORIES {
            std::fs::create_dir_all(root.join(dir))?;
        }

        // The copies, written from the bytes already read rather than by reading the game again:
        // a second read could catch a different version of the file than the one that was hashed.
        // Written before project.json, so a run that fails halfway leaves no project claiming to
        // have pinned an original it does not hold.
        let pinned = root.join("original/tree");
        for entry in ingested.archive.entries() {
            let destination = pinned.join(&entry.name);
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&destination, &entry.data)?;
        }

        let record = crate::tree::TreeRecord {
            root: game.to_string_lossy().to_string(),
            files: ingested.files.clone(),
            skipped: ingested.skipped.clone(),
            scanned: ingested.scanned,
            total_size: ingested.total_size,
            unread_files_are_not_hashed: true,
            evidence: ingested.evidence.clone(),
        };
        write_json(&root.join("original/tree.json"), &record)?;

        let detected = crate::detect::detect_source_language(&ingested.archive);

        let profile = ProjectProfile {
            schema_version: SCHEMA_VERSION,
            name: name.to_string(),
            revision: 0,
            source: Source::Tree {
                root: game.to_string_lossy().to_string(),
                sha256: crate::tree::manifest_sha256(&ingested.files),
            },
            source_language: SourceLanguage {
                language: detected.0,
                detected: true,
            },
            targets: vec![Target::new(Language::new("vi-VN"), "natural-dialogue")],
            branding: Branding::default(),
            permission_reference: None,
            provider: None,
            claude: None,
            text_assets: Vec::new(),
            font: None,
            emulator: None,
        };

        let mut project = Project { root, profile };
        project.save()?;
        Ok((project, ingested))
    }

    /// Opens an existing project and verifies the original has not been touched.
    pub fn open(root: impl AsRef<Path>) -> crate::Result<Self> {
        let root = root.as_ref().to_path_buf();
        let path = root.join("project.json");
        let text = std::fs::read_to_string(&path).map_err(|e| crate::Error::InvalidProject {
            path: root.clone(),
            reason: format!("cannot read project.json: {e}"),
        })?;
        let raw: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| crate::Error::InvalidProject {
                path: root.clone(),
                reason: format!("project.json is not valid: {e}"),
            })?;

        let version = raw
            .get("schemaVersion")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        // A newer schema may use fields this build would silently drop on the next save, taking
        // the translator's work with them. Refusing is the safe direction.
        if version > SCHEMA_VERSION {
            return Err(crate::Error::InvalidProject {
                path: root,
                reason: format!(
                    "project uses schema version {version} but this build understands up to {SCHEMA_VERSION}"
                ),
            });
        }

        let raw = if version < SCHEMA_VERSION {
            migrate(raw, version)
        } else {
            raw
        };

        let profile: ProjectProfile =
            serde_json::from_value(raw).map_err(|e| crate::Error::InvalidProject {
                path: root.clone(),
                reason: format!("project.json is not valid: {e}"),
            })?;

        let mut project = Project { root, profile };
        project.verify_original()?;

        // An older project is rewritten in the new shape on open, so the migration happens once
        // rather than on every read, and the file on disk matches what the tool now believes.
        if version < SCHEMA_VERSION {
            project.save()?;
        }
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

    /// The language the game is written in.
    pub fn source_language(&self) -> &Language {
        &self.profile.source_language.language
    }

    /// The targets that are switched on.
    pub fn active_targets(&self) -> Vec<&Target> {
        self.profile.targets.iter().filter(|t| t.enabled).collect()
    }

    /// The target for a language, if the project has one.
    pub fn target(&self, language: &Language) -> Option<&Target> {
        self.profile
            .targets
            .iter()
            .find(|t| t.language.same_language_as(language) || t.language == *language)
    }

    fn require_target(&self, language: &Language) -> crate::Result<&Target> {
        self.target(language)
            .ok_or_else(|| crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: format!("this project has no {language} target"),
            })
    }

    /// Adds a language, or returns the existing one unchanged.
    pub fn add_target(&mut self, language: Language, style_profile: &str) -> crate::Result<()> {
        if self.target(&language).is_some() {
            return Ok(());
        }
        self.profile
            .targets
            .push(Target::new(language, style_profile));
        self.save()
    }

    /// Removes a language from the profile.
    ///
    /// Its translations and builds are left on disk. Deleting a body of reviewed work because a
    /// checkbox was cleared is not a thing a tool should do; re-adding the language picks them
    /// straight back up.
    pub fn remove_target(&mut self, language: &Language) -> crate::Result<()> {
        self.profile
            .targets
            .retain(|t| !(t.language == *language || t.language.same_language_as(language)));
        self.save()
    }

    /// Re-hashes the original and refuses to continue if it changed.
    ///
    /// A tree is hashed through its manifest, so this stays one comparison for both kinds of
    /// source rather than becoming a loop with failure modes of its own.
    pub fn verify_original(&self) -> crate::Result<()> {
        let actual = match &self.profile.source {
            Source::Archive { .. } => sha256_hex(&self.original_bytes()?),
            // Re-hashed from the copies on disk, not read back out of tree.json. Comparing the
            // record with itself always passes, which is a check that checks nothing.
            Source::Tree { .. } => crate::tree::manifest_sha256(&self.pinned_now()?),
        };
        if actual != self.profile.source.sha256() {
            return Err(crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: format!(
                    "the original has been modified: project.json records {} but {} is {actual}",
                    self.profile.source.sha256(),
                    self.profile.source.label()
                ),
            });
        }
        Ok(())
    }

    /// What was pinned when a directory game was imported.
    pub fn tree_record(&self) -> crate::Result<crate::tree::TreeRecord> {
        read_json(&self.root.join("original/tree.json"))
    }

    /// The pinned originals as they are on disk right now, hashed afresh.
    ///
    /// A file that has gone missing hashes as absent rather than being skipped: a manifest that
    /// silently shrank would match nothing and say nothing about why.
    fn pinned_now(&self) -> crate::Result<Vec<crate::tree::Pinned>> {
        let root = self.root.join("original/tree");
        self.tree_record()?
            .files
            .into_iter()
            .map(|file| {
                let data = std::fs::read(root.join(&file.path)).unwrap_or_default();
                Ok(crate::tree::Pinned {
                    size: data.len() as u64,
                    sha256: sha256_hex(&data),
                    path: file.path,
                })
            })
            .collect()
    }

    /// The imported file's bytes. Only an archive source has any.
    pub fn original_bytes(&self) -> crate::Result<Vec<u8>> {
        match &self.profile.source {
            Source::Archive { jar, .. } => Ok(std::fs::read(self.root.join(jar))?),
            Source::Tree { .. } => Err(crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: "this project's game is a directory, so it has no single file".into(),
            }),
        }
    }

    /// The original, as the twenty-six functions downstream want it.
    ///
    /// For a directory this rebuilds the archive from the copies under `original/tree/` rather
    /// than from the game itself. That is the point of copying rather than only hashing: the
    /// original stays reachable byte for byte after Steam has updated over the game.
    pub fn original(&self) -> crate::Result<Archive> {
        match &self.profile.source {
            Source::Archive { .. } => Archive::read(&self.original_bytes()?),
            Source::Tree { .. } => {
                let record = self.tree_record()?;
                let pinned = self.root.join("original/tree");
                let mut archive = Archive::empty();
                for file in &record.files {
                    archive.insert(file.path.clone(), std::fs::read(pinned.join(&file.path))?);
                }
                Ok(archive)
            }
        }
    }

    /// Detects capabilities and writes the manifest (§22, step 4).
    /// What kind of package this is, and how far this tool can take it (§7).
    pub fn package(&self) -> crate::Result<crate::package::Detected> {
        let archive = self.original()?;
        match &self.profile.source {
            Source::Archive { .. } => Ok(crate::package::detect(&archive)),
            // A directory cannot be recognised from its entries once ingested - it looks like a
            // zip - so the kind is given and the engine evidence comes from the scan.
            Source::Tree { .. } => Ok(crate::package::detect_tree(
                &archive,
                self.tree_record()?.evidence,
            )),
        }
    }

    pub fn analyze(&self) -> crate::Result<CapabilityManifest> {
        let archive = self.original()?;
        let mut manifest = detect::detect(&archive);
        // What a plugin recognised goes in the same manifest as what the detectors did, each
        // carrying the plugin that claimed it. Kept apart it would be a second kind of truth
        // nothing downstream knew to ask about; merged without the evidence it would be
        // untraceable.
        manifest
            .capabilities
            .extend(self.plugins()?.capabilities(&archive));
        write_json(&self.root.join("extracted/capabilities.json"), &manifest)?;
        Ok(manifest)
    }

    /// Extracts the content graph (§22, step 5-6).
    ///
    /// Shared by every target: the source text is the same whatever it is being translated into.
    pub fn extract(&self) -> crate::Result<ContentGraph> {
        let graph = graph::extract_with(&self.original()?, &self.plugin_formats()?);
        write_json(&self.root.join("content/graph.json"), &graph)?;
        // Read straight away rather than on demand: the readings are about this graph, and a
        // stale set of them beside a fresh graph would attribute lines to characters who are no
        // longer in the game.
        write_json(
            &self.root.join("content/context.json"),
            &crate::context::infer(&graph),
        )?;
        Ok(graph)
    }

    /// Reads the graph for what its lines are and who speaks them (§10, §5, §15).
    ///
    /// Run as part of extraction, so anything that asks for a node's voice has an answer without
    /// a separate step somebody has to remember. It changes no node: the readings sit beside the
    /// graph, carry their evidence, and the graph's own classifications stand where the two
    /// disagree.
    pub fn infer_context(&self) -> crate::Result<crate::context::Inference> {
        let inference = crate::context::infer(&self.graph()?);
        write_json(&self.root.join("content/context.json"), &inference)?;
        Ok(inference)
    }

    /// The last inference, if extraction has run.
    pub fn inference(&self) -> crate::Result<crate::context::Inference> {
        let path = self.root.join("content/context.json");
        if !path.exists() {
            return Ok(crate::context::Inference::default());
        }
        read_json(&path)
    }

    /// The voice one node should be translated in (§14, §15).
    ///
    /// The game talking to the player unless something established otherwise, because that is
    /// what interface text is and what most of a game's strings are.
    pub fn voice(
        &self,
        node: &str,
    ) -> crate::Result<(crate::register::Speaker, crate::register::Stance)> {
        Ok(self.inference()?.voice(node))
    }

    /// The last scan's suggestions, kept apart from anything the package survey established.
    ///
    /// Stored under `content/` beside the graph, and read back separately for the same reason it
    /// is shown separately: a guess that got filed alongside a fact would, a week later, be
    /// indistinguishable from one.
    pub fn suggestions(&self) -> crate::Result<Option<crate::claude::Survey>> {
        let path = self.root.join("content/suggestions.json");
        if !path.exists() {
            return Ok(None);
        }
        read_json(&path).map(Some)
    }

    pub fn save_suggestions(&self, survey: &crate::claude::Survey) -> crate::Result<()> {
        write_json(&self.root.join("content/suggestions.json"), survey)
    }

    pub fn graph(&self) -> crate::Result<ContentGraph> {
        read_json(&self.root.join("content/graph.json"))
    }

    // ---- per-target paths -------------------------------------------------
    //
    // Everything a target owns is filed under its language slug, so adding a language cannot
    // disturb an existing one and removing one leaves the rest intact.

    fn translations_path(&self, target: &Target) -> PathBuf {
        self.root
            .join("translations")
            .join(format!("{}.json", target.slug()))
    }

    fn candidates_path(&self, target: &Target) -> PathBuf {
        self.root
            .join("translations")
            .join(format!("{}.candidates.json", target.slug()))
    }

    fn glossary_path(&self, target: &Target) -> PathBuf {
        self.root
            .join("glossary")
            .join(format!("{}.json", target.slug()))
    }

    fn memory_path(&self, target: &Target) -> PathBuf {
        self.root.join("memory").join(format!(
            "{}-{}.json",
            self.source_language()
                .tag()
                .to_lowercase()
                .replace(|c: char| !c.is_ascii_alphanumeric(), "-"),
            target.slug()
        ))
    }

    fn builds_dir(&self, target: &Target) -> PathBuf {
        self.root.join("builds").join(target.slug())
    }

    pub fn translations(&self, language: &Language) -> crate::Result<TranslationStore> {
        let target = self.require_target(language)?;
        read_json_or_default(&self.translations_path(target))
    }

    pub fn save_translations(
        &self,
        language: &Language,
        store: &TranslationStore,
    ) -> crate::Result<()> {
        let target = self.require_target(language)?;
        write_json(&self.translations_path(target), store)
    }

    pub fn glossary(&self, language: &Language) -> crate::Result<Glossary> {
        let target = self.require_target(language)?;
        read_json_or_default(&self.glossary_path(target))
    }

    pub fn save_glossary(&self, language: &Language, glossary: &Glossary) -> crate::Result<()> {
        let target = self.require_target(language)?;
        write_json(&self.glossary_path(target), glossary)
    }

    pub fn memory(&self, language: &Language) -> crate::Result<TranslationMemory> {
        let target = self.require_target(language)?;
        read_json_or_default(&self.memory_path(target))
    }

    pub fn save_memory(
        &self,
        language: &Language,
        memory: &TranslationMemory,
    ) -> crate::Result<()> {
        let target = self.require_target(language)?;
        write_json(&self.memory_path(target), memory)
    }

    /// The dictionary packs available to this project: the ones it carries plus any built in.
    pub fn dictionary(&self) -> crate::Result<Dictionary> {
        let mut dictionary = crate::dictionary_data::builtin();
        let dir = self.root.join("dictionary");
        if !dir.exists() {
            return Ok(add_plugin_terms(dictionary, &self.plugins()?));
        }
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        files.sort();
        for file in files {
            // A malformed pack a user dropped in should name itself rather than failing the whole
            // project, so the error carries the file.
            let pack: crate::dictionary::Pack =
                read_json(&file).map_err(|e| crate::Error::InvalidProject {
                    path: file.clone(),
                    reason: format!("dictionary pack is not valid: {e}"),
                })?;
            dictionary.add(pack);
        }
        Ok(add_plugin_terms(dictionary, &self.plugins()?))
    }

    /// The register this project writes a language in, if this build ships that profile.
    pub fn style(&self, language: &Language) -> Option<crate::register::StyleProfile> {
        self.target(language)
            .and_then(|t| crate::register::builtin(&t.style_profile))
    }

    /// What the game's font can draw, if the project has said which font that is.
    ///
    /// `None` means nobody has established it. That is a different answer from "it covers
    /// everything", and the caller has to keep them apart: a game drawing from a sheet nobody
    /// declared will show blanks, and reporting that as fine is how a localization ships broken.
    pub fn font_coverage(&self) -> crate::Result<Option<Coverage>> {
        // A ready rule that switches the game's font class to the handset's own answers this
        // before the profile does: after it runs the game is not drawing from a sheet at all, so
        // the sheet's coverage is an answer about something that will no longer happen.
        if self.switched_to_device_font()? {
            let mut covered: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
            covered.extend(font::vietnamese_required());
            return Ok(Some(Coverage::new(
                covered,
                "the handset's own font, by rule",
            )));
        }
        let Some(profile) = &self.profile.font else {
            return Ok(None);
        };
        if profile.device_font {
            let mut covered: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
            covered.extend(font::vietnamese_required());
            return Ok(Some(Coverage::new(covered, "device font")));
        }
        // A ready rule that installs the composed sheet changes the answer: the game will ship
        // drawing from that sheet, so reporting the original's coverage would fail a build that
        // is in fact fine. Only a rule that is switched on and whose conditions hold counts -
        // one written for a different version of the artwork does not.
        if let Some(order) = self.installed_font_order()? {
            return Ok(Some(Coverage::new(
                order,
                format!("{} (with the composed sheet installed)", profile.entry),
            )));
        }
        let sheet = self.font_sheet()?;
        Ok(sheet.map(|s| Coverage::new(s.order.clone(), profile.entry.clone())))
    }

    /// Whether a rule that is on, and fits, hands this game's text to the handset's font (§16).
    ///
    /// Only a rule that is switched on *and* whose conditions hold counts, exactly as for an
    /// installed sheet: one written against a version of the class the game no longer has changes
    /// nothing, and reporting otherwise would pass a build that ships blanks.
    pub fn switched_to_device_font(&self) -> crate::Result<bool> {
        let rules = self.rules()?;
        if rules.is_empty() {
            return Ok(false);
        }
        let archive = self.original()?;
        for rule in rules.iter().filter(|r| r.enabled) {
            if !rule
                .then
                .iter()
                .any(|a| matches!(a, crate::rules::Action::UseDeviceFont { .. }))
            {
                continue;
            }
            let plan = crate::rules::plan(std::slice::from_ref(rule), &archive, &self.root)?;
            if plan.first().is_some_and(|p| p.ready()) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The characters the game will draw once the rules have run, when a rule installs a sheet.
    ///
    /// Read from the sidecar written beside the composed image rather than from the image, because
    /// what a sheet covers is which character sits in which cell, and no PNG records that.
    fn installed_font_order(&self) -> crate::Result<Option<Vec<char>>> {
        let Some(profile) = &self.profile.font else {
            return Ok(None);
        };
        let rules = self.rules()?;
        if rules.is_empty() {
            return Ok(None);
        }
        let archive = self.original()?;

        for rule in rules.iter().filter(|r| r.enabled) {
            let installs = rule.then.iter().find_map(|action| match action {
                crate::rules::Action::ReplaceEntry { entry, from } if *entry == profile.entry => {
                    Some(from.clone())
                }
                _ => None,
            });
            let Some(from) = installs else { continue };

            let fits = crate::rules::plan(std::slice::from_ref(rule), &archive, &self.root)?
                .first()
                .map(|p| p.ready())
                .unwrap_or(false);
            if !fits {
                continue;
            }

            // The sidecar sits beside the image the rule installs, whatever it is called.
            let sidecar = self.root.join(&from).with_extension("json");
            let Ok(text) = std::fs::read_to_string(&sidecar) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if let Some(order) = value.get("order").and_then(|o| o.as_str()) {
                return Ok(Some(order.chars().collect()));
            }
        }
        Ok(None)
    }

    /// The game's glyph sheet, read from the original archive.
    pub fn font_sheet(&self) -> crate::Result<Option<Sheet>> {
        let Some(profile) = &self.profile.font else {
            return Ok(None);
        };
        if profile.device_font || profile.entry.is_empty() {
            return Ok(None);
        }
        let archive = self.original()?;
        let entry = archive
            .get(&profile.entry)
            .ok_or_else(|| crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: format!("the archive has no entry {}", profile.entry),
            })?;
        let image = Image::decode_png(&entry.data)?;

        let grid = match profile.grid {
            Some(grid) => grid,
            None => {
                return Err(crate::Error::InvalidProject {
                    path: self.root.clone(),
                    reason: "the font profile has no grid; a guessed one shifts every glyph".into(),
                })
            }
        };
        let order: Vec<char> = if profile.order.is_empty() {
            (0x20u8..=0x7E).map(|b| b as char).collect()
        } else {
            profile.order.chars().collect()
        };
        Ok(Some(Sheet::new(image, grid, order, [0, 0, 0, 0])))
    }

    /// How wide the game's own letters are, when it draws from a sheet (§24).
    ///
    /// `None` when nobody has said which image the font is, or when the game uses the device
    /// font. In both cases nothing here can measure anything, which is a different answer from
    /// "every label fits".
    pub fn font_metrics(&self) -> crate::Result<Option<font::metrics::Metrics>> {
        Ok(self
            .shipping_sheet()?
            .as_ref()
            .map(font::metrics::Metrics::of))
    }

    /// The sheet the game will actually draw from.
    ///
    /// The composed one where a rule installs it, the archive's own otherwise. Everything that
    /// measures or previews text goes through here, because the letters that ship are the letters
    /// a player will see, and measuring the ones being replaced would answer the wrong question.
    pub fn font_sheet_for_preview(&self) -> crate::Result<Option<font::sheet::Sheet>> {
        self.shipping_sheet()
    }

    fn shipping_sheet(&self) -> crate::Result<Option<font::sheet::Sheet>> {
        // Nothing to measure or draw with once the game has been switched to the handset's font:
        // its sheet is still in the archive and is no longer what the player sees. Measuring it
        // would answer a question about the build that was not made.
        if self.switched_to_device_font()? {
            return Ok(None);
        }
        let Some(sheet) = self.font_sheet()? else {
            return Ok(None);
        };
        if let Some(order) = self.installed_font_order()? {
            let composed = self.root.join("fonts/extended.png");
            if let Ok(bytes) = std::fs::read(&composed) {
                let image = Image::decode_png(&bytes)?;
                let rows = image.height / sheet.grid.cell_height;
                let grid = font::sheet::Grid { rows, ..sheet.grid };
                return Ok(Some(font::sheet::Sheet::new(
                    image,
                    grid,
                    order,
                    [0, 0, 0, 0],
                )));
            }
        }
        Ok(Some(sheet))
    }

    /// Draws every approved translation as the game will draw it, and writes the picture into the
    /// project (§25).
    ///
    /// Not an emulator, and it does not pretend to be: it cannot show a menu, a background or a
    /// button. It shows the text, in the game's own glyphs, at the game's own size, with a marker
    /// where the original ended - which is where the failures this tool can see actually live.
    /// `None` when the game has no sheet to draw with, or nothing has been approved yet.
    pub fn proof_sheet(&self, language: &Language, scale: u32) -> crate::Result<Option<PathBuf>> {
        let Some(image) = self.proof_image(language, scale)? else {
            return Ok(None);
        };
        let dir = self.root.join("tests");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("proof-{}.png", self.slug(language)));
        std::fs::write(&path, image.encode_png()?)?;
        Ok(Some(path))
    }

    /// The same drawing, in memory, for anything that wants to compare rather than look.
    pub fn proof_image(&self, language: &Language, scale: u32) -> crate::Result<Option<Image>> {
        let Some(sheet) = self.shipping_sheet()? else {
            return Ok(None);
        };
        let metrics = font::metrics::Metrics::of(&sheet);
        let graph = self.graph()?;
        let translations = self.translations(language)?;

        let pairs: Vec<(String, String)> = graph
            .nodes
            .iter()
            .filter_map(|node| {
                translations
                    .get(&node.id)
                    .map(|t| (node.source_text.clone(), t.to_string()))
            })
            .collect();
        if pairs.is_empty() {
            return Ok(None);
        }
        let rows: Vec<font::proof::Row> = pairs
            .iter()
            .map(|(source, target)| font::proof::Row { source, target })
            .collect();

        Ok(Some(font::proof::sheet(&sheet, &metrics, &rows, scale)))
    }

    fn slug(&self, language: &Language) -> String {
        language
            .tag()
            .to_lowercase()
            .replace(|c: char| !c.is_ascii_alphanumeric(), "-")
    }

    /// Where a language's accepted drawing is kept.
    pub fn baseline_path(&self, language: &Language) -> PathBuf {
        self.root
            .join("tests")
            .join(format!("baseline-{}.png", self.slug(language)))
    }

    /// Accepts the current drawing as what this language is supposed to look like (§25).
    ///
    /// A person looks at the picture and says yes. That is the only way a baseline can be
    /// established honestly: a baseline taken automatically records whatever the tool did last
    /// time, including whatever it did wrong.
    pub fn accept_baseline(
        &self,
        language: &Language,
        scale: u32,
    ) -> crate::Result<Option<PathBuf>> {
        let Some(image) = self.proof_image(language, scale)? else {
            return Ok(None);
        };
        let path = self.baseline_path(language);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, image.encode_png()?)?;
        Ok(Some(path))
    }

    /// Compares the current drawing against the accepted one (§25).
    ///
    /// `None` when there is nothing to draw or nothing to compare against, which are different
    /// situations from "nothing changed" and must not be reported as it.
    pub fn visual_regression(
        &self,
        language: &Language,
        scale: u32,
    ) -> crate::Result<Option<(crate::regress::Difference, PathBuf)>> {
        let path = self.baseline_path(language);
        if !path.exists() {
            return Ok(None);
        }
        let Some(after) = self.proof_image(language, scale)? else {
            return Ok(None);
        };
        let before = Image::decode_png(&std::fs::read(&path)?)?;
        let difference = crate::regress::compare(&before, &after);

        // The picture is written whether or not anything changed: a person who ran this because
        // something looked wrong wants to look, and an empty marked image is itself an answer.
        let marked = self
            .root
            .join("tests")
            .join(format!("changed-{}.png", self.slug(language)));
        std::fs::write(
            &marked,
            crate::regress::marked(&before, &after).encode_png()?,
        )?;
        Ok(Some((difference, marked)))
    }

    /// Runs the emulator this project's owner configured, on the newest build (§25).
    ///
    /// Their command, from their project file. Nothing here chooses it, and nothing read out of
    /// the game can influence it - §29's rule is that nothing extracted is executed, and a
    /// launcher taking its command from a manifest would break that rule while looking helpful.
    pub fn play(&self, language: &Language) -> crate::Result<std::process::ExitStatus> {
        let Some(emulator) = &self.profile.emulator else {
            return Err(crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: "this project has no emulator configured: set one with `tjlocalizer play \
                         <project> --command <program>`, and it will be used from then on"
                    .into(),
            });
        };
        let target = self
            .target(language)
            .ok_or_else(|| crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: format!("this project does not target {}", language.tag()),
            })?;
        let game = self.root.join("output").join(self.output_name(target));
        if !game.exists() {
            return Err(crate::Error::InvalidProject {
                path: game,
                reason: "there is no build to run yet: build this language first".into(),
            });
        }

        std::process::Command::new(&emulator.command)
            .args(emulator.arguments(&game))
            .status()
            .map_err(|e| crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: format!("could not run {}: {e}", emulator.command),
            })
    }

    /// Checks one language's approved translations against the game's font (§16, §24).
    pub fn font_report(&self, language: &Language) -> crate::Result<Option<CoverageReport>> {
        let Some(coverage) = self.font_coverage()? else {
            return Ok(None);
        };
        let translations = self.translations(language)?;
        let strings: Vec<&str> = translations.approved.values().map(|s| s.as_str()).collect();
        Ok(Some(font::report(&coverage, strings)))
    }

    /// Builds a sheet holding the game's own glyphs plus the Vietnamese letters composed from
    /// them, and writes it into the project's `fonts/` directory.
    ///
    /// It does not install the sheet. Making the game *use* the new glyphs means changing how it
    /// looks them up, which is per-game and belongs to the rule engine (§19); `font_install_rule`
    /// writes what it can and `font_lookup_candidates` looks for the rest. This produces the
    /// artwork and says so.
    pub fn compose_font(
        &self,
        marks: Option<&MarkSource>,
    ) -> crate::Result<Option<(PathBuf, font::sheet::Extension)>> {
        let Some(sheet) = self.font_sheet()? else {
            return Ok(None);
        };
        let (extended, report) =
            extend_with_marks(&sheet, &font::vietnamese_compositions(), marks)?;

        let dir = self.root.join("fonts");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("extended.png");
        std::fs::write(&path, extended.image.encode_png()?)?;

        // A sidecar rather than a format: J2ME games agree on no font metadata whatsoever, so
        // anything written into the sheet itself would be a guess about one game.
        write_json(
            &dir.join("extended.json"),
            &serde_json::json!({
                "source": self.profile.font.as_ref().map(|f| f.entry.clone()),
                "grid": extended.grid,
                "order": extended.order.iter().collect::<String>(),
                "added": report.added.iter().collect::<String>(),
                "marksFromTypeface": report.from_typeface,
                "typeface": report.typeface,
                "skipped": report.skipped,
                "note": "Glyphs only. Making the game use them requires changing its font lookup, which is per-game and is not done here.",
            }),
        )?;
        Ok(Some((path, report)))
    }

    /// Every image in the archive that could be a glyph sheet, best first.
    ///
    /// Ranked rather than chosen. What separates a glyph sheet from a sprite atlas is often only
    /// obvious to someone who has seen the game run.
    pub fn font_candidates(&self) -> crate::Result<Vec<font::sheet::SheetCandidate>> {
        let archive = self.original()?;
        let mut found = Vec::new();

        for entry in archive.entries() {
            if entry.extension() != "png" {
                continue;
            }
            // An image that fails to decode is not a candidate; it is also not an error worth
            // stopping for, because the archive is somebody else's and may hold anything.
            if let Ok(candidate) = font::sheet::inspect(&entry.name, &entry.data) {
                found.push(candidate);
            }
        }

        found.sort_by(|a, b| {
            let score = |c: &font::sheet::SheetCandidate| {
                let best_fit = c.grids.first().map(|g| g.fit).unwrap_or(0.0);
                // Few colours and little ink is what a glyph sheet looks like; a grid the glyphs
                // sit inside is what confirms it.
                let sparse = (1.0 - c.ink_share).clamp(0.0, 1.0);
                let plain = 1.0 - (c.colours as f32 / 512.0).clamp(0.0, 1.0);
                best_fit * 0.6 + sparse * 0.2 + plain * 0.2
            };
            score(b).total_cmp(&score(a))
        });
        Ok(found)
    }

    /// Renders sample text with the drawn marks and with the chosen typeface, for comparison.
    ///
    /// Which reads better is not something a count can answer. A typeface supplying more marks is
    /// not a typeface producing better ones - its diacritics are outlines rasterised small, and
    /// the drawn ones are shapes designed for this size. So the choice is put in front of a person
    /// at the size that ships.
    pub fn preview_font(&self, text: &[&str], scale: u32) -> crate::Result<Option<PathBuf>> {
        let Some(sheet) = self.font_sheet()? else {
            return Ok(None);
        };
        let compositions = font::vietnamese_compositions();

        let (drawn, _) = extend_with_marks(&sheet, &compositions, None)?;
        let chosen = self
            .profile
            .font
            .as_ref()
            .and_then(|f| f.marks_from.clone())
            .map(|path| MarkSource::from_path(&path))
            .transpose()?;

        let mut sheets: Vec<(&str, &font::sheet::Sheet)> = vec![("drawn", &drawn)];
        let borrowed;
        if let Some(source) = chosen.as_ref() {
            borrowed = extend_with_marks(&sheet, &compositions, Some(source))?.0;
            sheets.push(("typeface", &borrowed));
        }

        let image = font::sheet::preview(&sheets, text, scale);
        let dir = self.root.join("fonts");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("preview.png");
        std::fs::write(&path, image.encode_png()?)?;
        Ok(Some(path))
    }

    /// Every image in the game, with what its shape suggests about words painted into it (§17).
    pub fn image_assets(&self) -> crate::Result<Vec<crate::assets::ImageAsset>> {
        crate::assets::scan(&self.original()?)
    }

    /// Reads the words out of images, with the game's own glyph sheet (§17).
    ///
    /// `None` when the project has not said which image the font is, or when the game uses the
    /// device font: in both cases there are no letters to match against, which is a different
    /// answer from "the images say nothing".
    ///
    /// Given no entries, every image whose shape suggests a label is read. Named entries are read
    /// whatever their shape, because a person naming one has already decided it is worth looking
    /// at and is owed an answer rather than a filter.
    pub fn read_text_assets(
        &self,
        entries: &[String],
    ) -> crate::Result<Option<Vec<crate::assets::ocr::Reading>>> {
        let Some(sheet) = self.font_sheet()? else {
            return Ok(None);
        };
        Ok(Some(crate::assets::read(
            &self.original()?,
            &sheet,
            entries,
        )?))
    }

    /// Records that an image carries words, or updates what is known about one.
    pub fn mark_text_asset(&mut self, asset: crate::assets::TextAsset) -> crate::Result<()> {
        let archive = self.original()?;
        if archive.get(&asset.entry).is_none() {
            return Err(crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: format!("the archive has no entry {}", asset.entry),
            });
        }
        match self
            .profile
            .text_assets
            .iter_mut()
            .find(|a| a.entry == asset.entry)
        {
            Some(existing) => *existing = asset,
            None => self.profile.text_assets.push(asset),
        }
        self.save()
    }

    pub fn unmark_text_asset(&mut self, entry: &str) -> crate::Result<bool> {
        let before = self.profile.text_assets.len();
        self.profile.text_assets.retain(|a| a.entry != entry);
        let removed = self.profile.text_assets.len() != before;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// The adapters this project carries (§20).
    ///
    /// Loaded on every call rather than cached: a person editing a plugin file expects the next
    /// command to see it, and these are a handful of small JSON files.
    pub fn plugins(&self) -> crate::Result<crate::plugin::Plugins> {
        crate::plugin::Plugins::load(&crate::plugin::dir(&self.root))
    }

    /// The file formats plugins claim, in the shape extraction and the build both take.
    pub fn plugin_formats(&self) -> crate::Result<crate::plugin::Formats> {
        Ok(self.plugins()?.formats())
    }

    /// The per-game patches this project holds (§19).
    /// The rules this project can run: the ones it wrote down, and the ones its plugins offer.
    ///
    /// A plugin's rule is offered, never applied - it arrives switched off like every other rule,
    /// and switching it on writes it into the project's own `rules/rules.json`, where it is then
    /// this project's rule. A plugin cannot change a rule somebody already accepted: an id the
    /// project holds wins over the same id from a plugin.
    pub fn rules(&self) -> crate::Result<Vec<crate::rules::Rule>> {
        let mut rules = crate::rules::load(&self.root)?;
        for offered in self.plugins()?.rules() {
            if !rules.iter().any(|r| r.id == offered.id) {
                rules.push(offered);
            }
        }
        Ok(rules)
    }

    pub fn save_rules(&self, rules: &[crate::rules::Rule]) -> crate::Result<()> {
        crate::rules::save(&self.root, rules)
    }

    /// What each rule would do to this game, or why it cannot.
    pub fn plan_rules(&self) -> crate::Result<Vec<crate::rules::RulePlan>> {
        crate::rules::plan(&self.rules()?, &self.original()?, &self.root)
    }

    /// Adds or replaces one rule by id, leaving the others alone.
    pub fn put_rule(&self, rule: crate::rules::Rule) -> crate::Result<()> {
        let mut rules = self.rules()?;
        match rules.iter_mut().find(|r| r.id == rule.id) {
            Some(existing) => *existing = rule,
            None => rules.push(rule),
        }
        self.save_rules(&rules)
    }

    pub fn remove_rule(&self, id: &str) -> crate::Result<bool> {
        let mut rules = self.rules()?;
        let before = rules.len();
        rules.retain(|r| r.id != id);
        let removed = rules.len() != before;
        if removed {
            self.save_rules(&rules)?;
        }
        Ok(removed)
    }

    pub fn set_rule_enabled(&self, id: &str, enabled: bool) -> crate::Result<bool> {
        let mut rules = self.rules()?;
        let Some(rule) = rules.iter_mut().find(|r| r.id == id) else {
            return Ok(false);
        };
        rule.enabled = enabled;
        self.save_rules(&rules)?;
        Ok(true)
    }

    /// Writes the rule that puts the composed sheet into the game, as far as that can be known.
    ///
    /// Half a rule, honestly labelled. Replacing the image is the part that is the same in every
    /// game, and the conditions make it refuse if the artwork is not the one that was measured.
    /// The other part - teaching the game that the sheet now has more rows and which character
    /// each new cell holds - lives in code that differs per game, so this fills in what it can
    /// and leaves the rest for a person to add as `setIntConstant` or `setStringConstant`.
    ///
    /// It refuses to pretend otherwise. A rule that replaced the image and stopped would leave a
    /// game drawing its old letters from a taller sheet, which is a display bug rather than a
    /// missing feature, and much harder to see.
    /// How this game draws its text, from what its classes call (§16).
    ///
    /// The question that decides which of the two routes to Vietnamese is open. A game already
    /// calling `Graphics.drawString` needs no font work at all; one blitting pieces of an image
    /// needs either a composed sheet or the switch below.
    pub fn font_strategy(&self) -> crate::Result<crate::font::device::Strategy> {
        crate::font::device::strategy(&self.original()?)
    }

    /// The methods that could be switched to the handset's own font (§16).
    pub fn system_font_candidates(&self) -> crate::Result<Vec<crate::font::device::Candidate>> {
        crate::font::device::candidates(&self.original()?)
    }

    /// Writes the rules that switch this game to the handset's own font, all switched off.
    ///
    /// One rule per class rather than one per method, because switching a font class's drawing
    /// method and leaving its width method measuring the old sheet gives a game that draws
    /// correct text in the wrong places - the two belong to one decision.
    ///
    /// Nothing is applied. Every rule states which methods it would rewrite and stays off, because
    /// this trades the game's own lettering for the handset's and that is not a judgement this
    /// tool is in a position to make.
    pub fn write_system_font_rules(&self) -> crate::Result<Vec<crate::rules::Rule>> {
        use crate::rules::{Action, Condition, Rule};

        let candidates = self.system_font_candidates()?;
        if candidates.is_empty() {
            return Err(crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: "nothing in this game looks like a font class drawing from a sheet: no \
                         class both touches a Graphics and has a method shaped like drawing text"
                    .into(),
            });
        }

        let mut classes: Vec<String> = candidates.iter().map(|c| c.class.clone()).collect();
        classes.sort();
        classes.dedup();

        let mut written = Vec::new();
        for class in classes {
            let mine: Vec<&crate::font::device::Candidate> =
                candidates.iter().filter(|c| c.class == class).collect();
            let slug = class
                .trim_end_matches(".class")
                .replace(|c: char| !c.is_ascii_alphanumeric(), "-")
                .to_lowercase();

            let mut rule = Rule::new(
                format!("system-font-{slug}"),
                format!(
                    "Let the handset draw the text instead of {class}: {}. The game stops using \
                     its glyph sheet for this text, so Vietnamese needs no letters composed - and \
                     the game's own lettering is replaced by the handset's, which is a visible \
                     change. Found from what the class calls, not verified against a running game.",
                    mine.iter()
                        .map(|c| format!("{}{}", c.method, c.descriptor))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
            // The class has to be the one this was read from. A game updated underneath the
            // project would otherwise have a body written into whatever method now has that name.
            rule.when = vec![Condition::EntrySha256 {
                entry: class.clone(),
                sha256: sha256_hex(
                    &self
                        .original()?
                        .get(&class)
                        .expect("the candidate came from this archive")
                        .data,
                ),
            }];
            rule.then = mine
                .iter()
                .map(|c| Action::UseDeviceFont {
                    class: class.clone(),
                    method: c.method.clone(),
                    descriptor: c.descriptor.clone(),
                })
                .collect();

            self.put_rule(rule.clone())?;
            written.push(rule);
        }
        Ok(written)
    }

    /// Where in the game's code the shape of its glyph sheet appears to be written down (§16).
    ///
    /// The half of a font swap this tool could not do: replacing the image is the same in every
    /// game, and telling the game the sheet is now taller - and which character each new cell
    /// holds - is per-game code. It cannot be inferred. It can, however, be *looked for*, and
    /// what turns up is far more useful than an empty box: a class holding the number 16 when the
    /// sheet has 16 columns, or a string listing the sheet's characters in the sheet's order, is
    /// almost always the lookup.
    ///
    /// Evidence, never a decision. Every candidate says what was found and where, a person reads
    /// it against the game they know, and nothing is patched until they enable the rule.
    pub fn font_lookup_candidates(&self) -> crate::Result<Vec<FontLookup>> {
        let Some(sheet) = self.font_sheet()? else {
            return Ok(Vec::new());
        };
        let archive = self.original()?;
        let order: String = sheet.order.iter().collect();

        let mut found = Vec::new();
        for entry in archive.classes() {
            let Ok(class) = crate::classfile::ClassFile::parse(&entry.data) else {
                continue;
            };

            for (_, value) in class.integers() {
                let what = if value == sheet.grid.columns as i32 {
                    LookupEvidence::Columns
                } else if value == sheet.grid.rows as i32 {
                    LookupEvidence::Rows
                } else if value == sheet.grid.cell_width as i32 {
                    LookupEvidence::CellWidth
                } else if value == sheet.grid.cell_height as i32 {
                    LookupEvidence::CellHeight
                } else {
                    continue;
                };
                found.push(FontLookup {
                    class: entry.name.clone(),
                    what,
                    value: value.to_string(),
                });
            }

            for literal in class.string_literals() {
                let Some(text) = literal.decoded else {
                    continue;
                };
                // The character order, as the game lists it. Matched as a run of the sheet's own
                // characters in the sheet's own order rather than by equality, because a game
                // usually lists the part of the sheet it uses rather than all of it.
                if text.chars().count() >= 16 && order.contains(&text) {
                    found.push(FontLookup {
                        class: entry.name.clone(),
                        what: LookupEvidence::Order,
                        value: text,
                    });
                }
            }
        }
        found.sort_by(|a, b| a.class.cmp(&b.class).then(a.value.cmp(&b.value)));
        found.dedup_by(|a, b| a.class == b.class && a.what == b.what && a.value == b.value);
        Ok(found)
    }

    pub fn font_install_rule(&self) -> crate::Result<crate::rules::Rule> {
        use crate::rules::{Action, Condition, Rule};

        let profile = self
            .profile
            .font
            .as_ref()
            .ok_or_else(|| crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: "say which image holds the font first".into(),
            })?;
        if profile.device_font || profile.entry.is_empty() {
            return Err(crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: "a game drawing with the device font has no sheet to install".into(),
            });
        }
        let composed = self.root.join("fonts/extended.png");
        if !composed.exists() {
            return Err(crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: "compose the font first - there is nothing to install yet".into(),
            });
        }

        let original = self.original()?;
        let current = original
            .get(&profile.entry)
            .ok_or_else(|| crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: format!("the archive has no entry {}", profile.entry),
            })?;

        // What the composed sheet became, from the sidecar written beside it: the row count and
        // the character order both change when letters are added, and both are numbers the game
        // is likely to hold somewhere.
        let composed_shape: serde_json::Value =
            read_json(&self.root.join("fonts/extended.json")).unwrap_or_default();
        let new_rows = composed_shape
            .get("grid")
            .and_then(|g| g.get("rows"))
            .and_then(|r| r.as_u64())
            .unwrap_or(0) as i32;
        let new_order = composed_shape
            .get("order")
            .and_then(|o| o.as_str())
            .unwrap_or_default()
            .to_string();

        let candidates = self.font_lookup_candidates()?;
        let mut proposed: Vec<Action> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        for candidate in &candidates {
            match candidate.what {
                LookupEvidence::Rows if new_rows > 0 => {
                    let Ok(from) = candidate.value.parse::<i32>() else {
                        continue;
                    };
                    if from == new_rows {
                        continue;
                    }
                    proposed.push(Action::SetIntConstant {
                        class: candidate.class.clone(),
                        from,
                        to: new_rows,
                    });
                    notes.push(format!(
                        "{} holds {from}, which is the sheet's row count",
                        candidate.class
                    ));
                }
                LookupEvidence::Order if !new_order.is_empty() => {
                    // The game lists the part of the sheet it uses; the composed sheet keeps that
                    // run at the front and adds to the end, so the new listing is the old one
                    // plus what was added.
                    let extended = new_order
                        .strip_prefix(candidate.value.as_str())
                        .map(|rest| format!("{}{rest}", candidate.value));
                    let Some(extended) = extended else { continue };
                    if extended == candidate.value {
                        continue;
                    }
                    proposed.push(Action::SetStringConstant {
                        class: candidate.class.clone(),
                        from: candidate.value.clone(),
                        to: extended,
                    });
                    notes.push(format!(
                        "{} holds a string listing the sheet's characters in order",
                        candidate.class
                    ));
                }
                _ => {}
            }
        }

        let description = if proposed.is_empty() {
            format!(
                "Replace {} with the sheet holding the Vietnamese letters. The game must also be told the sheet is taller and which character each new cell holds; nothing in this game looked like where that is written, so that part is left for a person to add.",
                profile.entry
            )
        } else {
            format!(
                "Replace {} with the sheet holding the Vietnamese letters, and change what looks like the game's own record of the sheet's shape: {}. Read those against the game you know - they are what was found, not what was verified.",
                profile.entry,
                notes.join("; ")
            )
        };

        let mut rule = Rule::new("install-font", description);
        rule.when = vec![
            // By hash rather than by name: a rule written against one version of the artwork must
            // not run against another, because the composed sheet was measured from that one.
            Condition::EntrySha256 {
                entry: profile.entry.clone(),
                sha256: sha256_hex(&current.data),
            },
            Condition::ProjectFile {
                path: "fonts/extended.png".into(),
            },
        ];
        rule.then = vec![Action::ReplaceEntry {
            entry: profile.entry.clone(),
            from: "fonts/extended.png".into(),
        }];
        rule.then.extend(proposed);
        Ok(rule)
    }

    /// Whether the game on disk still matches what this project was built against.
    ///
    /// Its own check with its own name, rather than folded into `verify_original`, because it is a
    /// different fact about a different thing: `verify_original` says the pinned copies are
    /// intact, and this says the game they were copied *from* has moved on. Steam updating over a
    /// game is the quietest way this work goes wrong - nothing errors, the patch simply stops
    /// fitting - so it gets a name a person can look up.
    ///
    /// Only the files that were read are checked. The rest were never hashed, and `tree.json`
    /// says so.
    pub fn check_drift(&self) -> crate::Result<Vec<crate::validate::Finding>> {
        let record = self.tree_record()?;
        let game = std::path::Path::new(&record.root);
        if !game.is_dir() {
            return Ok(vec![crate::validate::Finding {
                severity: crate::validate::Severity::Warning,
                check: "tree.drift".into(),
                detail: format!(
                    "the game is no longer at {}, so nothing here could be compared with it; the \
                     project still holds its own copies",
                    record.root
                ),
            }]);
        }

        let mut moved = Vec::new();
        for file in &record.files {
            let data = std::fs::read(game.join(&file.path)).unwrap_or_default();
            if sha256_hex(&data) != file.sha256 {
                moved.push(file.path.clone());
            }
        }
        if moved.is_empty() {
            return Ok(Vec::new());
        }

        // One finding for the lot, with the count and a few names. One per file would be a
        // hundred identical lines after a game update, which is a report nobody reads.
        let sample: Vec<&str> = moved.iter().take(3).map(|p| p.as_str()).collect();
        Ok(vec![crate::validate::Finding {
            severity: crate::validate::Severity::Warning,
            check: "tree.drift".into(),
            detail: format!(
                "{} of the {} files this project read have changed in the game since it was \
                 imported ({}{}); the game was probably updated, and a patch built now will not \
                 apply to it",
                moved.len(),
                record.files.len(),
                sample.join(", "),
                if moved.len() > sample.len() {
                    ", ..."
                } else {
                    ""
                },
            ),
        }])
    }

    /// The patch a build produced, if this project builds patches.
    pub fn patch_dir(&self, target: &Target) -> Option<PathBuf> {
        if !self.profile.source.is_tree() {
            return None;
        }
        let path = self.root.join("output").join(self.output_name(target));
        path.is_dir().then_some(path)
    }

    /// Applies the current patch to a game directory, keeping what it replaced.
    ///
    /// The most destructive thing this tool does, so it is never a side effect of anything: the
    /// caller has to ask for it by name, and it refuses the whole patch rather than writing part
    /// of one.
    pub fn apply_patch(
        &self,
        language: &Language,
        game: &std::path::Path,
    ) -> crate::Result<Vec<String>> {
        let target = self.require_target(language)?;
        let patch = self
            .patch_dir(target)
            .ok_or_else(|| crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: "there is no patch to apply; build first".into(),
            })?;
        let manifest = crate::patch::read(&patch)?;

        // Beside the build it came from, because that is where this project keeps every version
        // of everything and where rollback already looks.
        let backup = self
            .builds_dir(target)
            .join(format!("{:04}", manifest.revision))
            .join("backup");
        std::fs::create_dir_all(&backup)?;
        crate::patch::apply(&manifest, game, &patch, &backup)
    }

    /// What applying the current patch would overwrite, without writing anything.
    pub fn plan_patch(
        &self,
        language: &Language,
        game: &std::path::Path,
    ) -> crate::Result<crate::patch::Plan> {
        let target = self.require_target(language)?;
        let patch = self
            .patch_dir(target)
            .ok_or_else(|| crate::Error::InvalidProject {
                path: self.root.clone(),
                reason: "there is no patch to apply; build first".into(),
            })?;
        Ok(crate::patch::plan(&crate::patch::read(&patch)?, game))
    }

    /// Shorter ways of saying what one node's translation says (§24).
    ///
    /// Offered after the layout check reports a label will not fit, and never applied: every
    /// alternative comes from something this project already holds - another reading in its
    /// dictionary, a word its own interface register says to drop - and a person picks.
    pub fn shorter_alternatives(
        &self,
        language: &Language,
        node_id: &str,
    ) -> crate::Result<Vec<crate::shorten::Alternative>> {
        let graph = self.graph()?;
        let Some(node) = graph.nodes.iter().find(|n| n.id == node_id) else {
            return Ok(Vec::new());
        };
        let translations = self.translations(language)?;
        let Some(current) = translations.get(node_id) else {
            return Ok(Vec::new());
        };
        let dictionary = self.dictionary()?;
        let metrics = self.font_metrics()?;

        Ok(crate::shorten::alternatives(
            &node.source_text,
            current,
            &dictionary,
            self.source_language(),
            language,
            node.context.key(),
            metrics.as_ref(),
        ))
    }

    /// Generates translation candidates for one target (§22, step 9).
    pub fn suggest(
        &self,
        language: &Language,
        fuzzy_threshold: f32,
    ) -> crate::Result<CandidateSet> {
        let target = self.require_target(language)?;
        let set = suggest::candidates(
            &self.graph()?,
            &self.memory(language)?,
            &self.glossary(language)?,
            &self.translations(language)?,
            fuzzy_threshold,
        );
        write_json(&self.candidates_path(target), &set)?;
        Ok(set)
    }

    pub fn candidates(&self, language: &Language) -> crate::Result<CandidateSet> {
        let target = self.require_target(language)?;
        read_json_or_default(&self.candidates_path(target))
    }

    /// Folds approved translations back into this direction's memory.
    pub fn learn(&self, language: &Language) -> crate::Result<usize> {
        let mut memory = self.memory(language)?;
        suggest::learn(&self.graph()?, &self.translations(language)?, &mut memory);
        let count = memory.entries.len();
        self.save_memory(language, &memory)?;
        Ok(count)
    }

    /// Builds, validates, records and publishes one target (§22 steps 15-18, §23).
    pub fn build(&self, language: &Language) -> crate::Result<BuildRecord> {
        self.verify_original()?;
        let target = self.require_target(language)?;
        let original = self.original()?;
        let graph = self.graph()?;
        let translations = self.translations(language)?;

        // Attribution is not written into a game that is a folder. In a JAR it is two files under
        // META-INF/, which is the archive's own bookkeeping; in a Steam install it is a stray
        // directory appearing in the middle of somebody's game, and some games check their own
        // directory for exactly that. It travels in the patch instead.
        let branding = if self.profile.source.is_tree() {
            Branding {
                enabled: false,
                ..self.profile.branding.clone()
            }
        } else {
            self.profile.branding.clone()
        };
        let (mut built, report) = build::apply_with(
            &original,
            &graph,
            &translations,
            &branding,
            &self.plugin_formats()?,
        )?;

        // Rules run last, on the archive the text has already been patched into. They are the
        // per-game part of the work - installing a font sheet, changing a layout constant - and
        // running them here means the validation below sees what will actually ship.
        let rules = crate::rules::apply(&self.rules()?, &mut built, &self.root)?;
        let bytes = built.write()?;
        let font = self.font_coverage()?;
        // Measured from the sheet the game actually draws from, which is the composed one when a
        // rule installs it: the letters that ship are the letters whose widths matter.
        let metrics = self.font_metrics()?;
        let package = crate::package::detect(&original);
        let mut validation = crate::validate::validate(&crate::validate::Subject {
            original: &original,
            built: &built,
            graph: &graph,
            translations: &translations,
            from: self.source_language(),
            to: &target.language,
            kind: package.kind,
            font: font.as_ref(),
            metrics: metrics.as_ref(),
        });

        // A package this tool cannot sign is not a failure and not a success either. Producing it
        // silently would hand somebody a file their device refuses to install, with nothing on
        // screen saying why - so the reason travels with the build record.
        if let Some(note) = package.kind.repackaging_note() {
            validation.extend([crate::validate::Finding {
                severity: crate::validate::Severity::Warning,
                check: "package.signature".into(),
                detail: format!("this is an {}: {note}", package.kind.label().to_lowercase()),
            }]);
        }

        validation.extend(crate::validate::check_refusals(&report.refused));

        // Run against the built archive, and after the rules: whether a redrawn image reached the
        // output is a fact about what will ship, not about what was intended.
        validation.extend(crate::validate::check_text_assets(
            &self.profile.text_assets,
            &self.root,
            &built,
        ));

        // Whether the game on disk is still the one this project was built against. Its own check
        // rather than part of verify_original, because it is a different fact: the pinned copies
        // are intact - that is what verify_original said - and the game they came from has moved
        // on. Steam updating over a game is the quietest way this work goes wrong.
        if self.profile.source.is_tree() {
            validation.extend(self.check_drift()?);
        }

        let revision = self.next_build_revision(target)?;
        let dir = self.builds_dir(target).join(format!("{revision:04}"));
        std::fs::create_dir_all(&dir)?;

        let name = self.output_name(target);
        if self.profile.source.is_tree() {
            // Only what changed. Copying a whole game install into the project to change three
            // strings is not a build, it is a second copy of somebody's game.
            let mut manifest = crate::patch::Manifest {
                project: self.profile.name.clone(),
                language: target.language.tag().to_string(),
                revision,
                changes: Vec::new(),
                localized_by: self
                    .profile
                    .branding
                    .enabled
                    .then(|| self.profile.branding.author.clone()),
            };
            crate::patch::write(&dir.join("patch"), &original, &built, &mut manifest)?;
        } else {
            std::fs::write(dir.join(&name), &bytes)?;
        }

        let record = BuildRecord {
            revision,
            language: target.language.clone(),
            profile_revision: self.profile.revision,
            source_sha256: self.profile.source.sha256().to_string(),
            translations_applied: translations.len(),
            report,
            rules,
            validation,
        };
        write_json(&dir.join("build.json"), &record)?;

        // Written under builds/ first and copied to output/ second, so output/ only ever holds a
        // build that finished and has a record.
        std::fs::create_dir_all(self.root.join("output"))?;
        if self.profile.source.is_tree() {
            copy_tree(&dir.join("patch"), &self.root.join("output").join(&name))?;
        } else {
            std::fs::write(self.root.join("output").join(&name), &bytes)?;
        }
        Ok(record)
    }

    /// Builds every enabled target.
    ///
    /// Each is independent, so one language failing validation does not stop the others: the
    /// caller gets a record per language and decides what to ship.
    pub fn build_all(&self) -> crate::Result<Vec<BuildRecord>> {
        let languages: Vec<Language> = self
            .active_targets()
            .iter()
            .map(|t| t.language.clone())
            .collect();
        languages.iter().map(|l| self.build(l)).collect()
    }

    /// Restores a previous build's output as the current one for that language.
    pub fn rollback(&self, language: &Language, revision: u32) -> crate::Result<BuildRecord> {
        let target = self.require_target(language)?;
        let dir = self.builds_dir(target).join(format!("{revision:04}"));
        let record: BuildRecord =
            read_json(&dir.join("build.json")).map_err(|_| crate::Error::InvalidProject {
                path: dir.clone(),
                reason: format!("no build {revision} to roll back to"),
            })?;
        let name = self.output_name(target);
        let bytes = std::fs::read(dir.join(&name))?;
        std::fs::write(self.root.join("output").join(&name), bytes)?;
        Ok(record)
    }

    /// Recorded builds for one language, oldest first.
    pub fn builds(&self, language: &Language) -> crate::Result<Vec<BuildRecord>> {
        let target = self.require_target(language)?;
        let dir = self.builds_dir(target);
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

    /// The localized artifact's file name for one target.
    pub fn output_name(&self, target: &Target) -> String {
        match &self.profile.source {
            Source::Archive { jar, .. } => {
                let extension = std::path::Path::new(jar)
                    .extension()
                    .map(|e| e.to_string_lossy().to_string())
                    .unwrap_or_else(|| "jar".into());
                format!("{}-{}.{extension}", self.profile.name, target.slug())
            }
            // A directory game builds to a directory of changed files, so there is no extension
            // to keep - and offering one would name a folder as though it were a file.
            Source::Tree { .. } => format!("{}-{}", self.profile.name, target.slug()),
        }
    }

    /// Where the last build for a language was published, if it is there.
    pub fn output_path(&self, language: &Language) -> crate::Result<Option<PathBuf>> {
        let target = self.require_target(language)?;
        let path = self.root.join("output").join(self.output_name(target));
        Ok(path.exists().then_some(path))
    }

    fn next_build_revision(&self, target: &Target) -> crate::Result<u32> {
        Ok(self
            .builds(&target.language)?
            .iter()
            .map(|b| b.revision)
            .max()
            .unwrap_or(0)
            + 1)
    }
}

/// Brings an older project.json into the current shape.
///
/// Version 2 had one `localization` object; version 3 has a source language and a list of
/// targets. The old fields are read and translated rather than dropped, so a project started
/// before multi-language support keeps its target, its register and all of its work.
fn migrate(mut raw: serde_json::Value, from_version: u32) -> serde_json::Value {
    if from_version < 3 {
        let localization = raw.get("localization").cloned().unwrap_or_default();
        let target_language = localization
            .get("targetLanguage")
            .and_then(|v| v.as_str())
            .unwrap_or("vi-VN")
            .to_string();
        let style = localization
            .get("styleProfile")
            .and_then(|v| v.as_str())
            .unwrap_or("natural-dialogue")
            .to_string();
        let source_language = localization
            .get("sourceLanguage")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        if let Some(object) = raw.as_object_mut() {
            object.remove("localization");
            object.insert(
                "sourceLanguage".into(),
                serde_json::json!({
                    // "auto" was version 2's way of saying "not decided", which version 3 writes
                    // as the undetermined tag so it reads as a language everywhere else.
                    "language": if source_language == "auto" { "und" } else { source_language },
                    "detected": source_language == "auto",
                }),
            );
            object.insert(
                "targets".into(),
                serde_json::json!([{
                    "language": target_language,
                    "styleProfile": style,
                    "enabled": true,
                }]),
            );
        }
    }

    // Version 4 made the source a tagged union. A version 3 source is the archive variant; saying
    // so here is the whole migration.
    if from_version < 4 {
        if let Some(source) = raw.get_mut("source").and_then(|s| s.as_object_mut()) {
            source.insert("kind".into(), serde_json::json!("archive"));
        }
    }

    // Version 5 only added optional fields, so an older project needs nothing done to it: an
    // absent emulator is a project whose owner has not said, which is the truth about it.

    // Stamped once, outside the per-version blocks. It used to sit inside the version 3 block,
    // so a version 3 file came out of here still labelled 3 - harmless only because `save`
    // rewrites the struct afterwards.
    if let Some(object) = raw.as_object_mut() {
        object.insert("schemaVersion".into(), serde_json::json!(SCHEMA_VERSION));
    }
    raw
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

/// Copies a directory tree, replacing whatever was at the destination.
///
/// Used to publish a patch into `output/`, which holds only the most recent build - so the old one
/// is removed first rather than merged into, or a file that stopped being changed would linger
/// there and be applied.
fn copy_tree(from: &Path, to: &Path) -> crate::Result<()> {
    if to.exists() {
        std::fs::remove_dir_all(to)?;
    }
    for entry in walkdir::WalkDir::new(from)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(from) else {
            continue;
        };
        let destination = to.join(relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(entry.path(), destination)?;
    }
    Ok(())
}

/// Adds what a project's plugins say its engine's words mean (§12, §20).
///
/// Listed after the project's own packs, because a tie between two readings goes to whichever was
/// listed first: a term the people on this project decided about should not be re-decided by an
/// adapter somebody downloaded.
fn add_plugin_terms(mut dictionary: Dictionary, plugins: &crate::plugin::Plugins) -> Dictionary {
    for pack in plugins.dictionary_packs() {
        dictionary.add(pack);
    }
    dictionary
}
