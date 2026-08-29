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
use crate::graph::{self, ContentGraph};
use crate::jar::{sha256_hex, Archive};
use crate::lang::Language;
use crate::provider::ProviderConfig;
use crate::suggest::{self, CandidateSet};
use crate::translation::{Glossary, TranslationMemory, TranslationStore};
use crate::validate::{validate, ValidationReport};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The project.json schema this build reads and writes.
///
/// Version 3 replaced the single `localization` object with a list of targets, so one project can
/// be shipped in several languages from one body of extracted text. A version 2 project is
/// migrated on open rather than refused: its one target becomes the first entry.
pub const SCHEMA_VERSION: u32 = 3;

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

        let jar_path = format!("original/{name}.jar");
        std::fs::write(root.join(&jar_path), jar)?;

        // Guessing the source language from the archive beats defaulting to English: a wrong
        // source language silently disables every dictionary, and the guess is recorded as a
        // guess so a person can see it was never confirmed.
        let detected = crate::detect::detect_source_language(&archive);

        let profile = ProjectProfile {
            schema_version: SCHEMA_VERSION,
            name: name.to_string(),
            revision: 0,
            source: Source {
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
    /// Shared by every target: the source text is the same whatever it is being translated into.
    pub fn extract(&self) -> crate::Result<ContentGraph> {
        let graph = graph::extract(&self.original()?);
        write_json(&self.root.join("content/graph.json"), &graph)?;
        Ok(graph)
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
            return Ok(dictionary);
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
        Ok(dictionary)
    }

    /// The register this project writes a language in, if this build ships that profile.
    pub fn style(&self, language: &Language) -> Option<crate::register::StyleProfile> {
        self.target(language)
            .and_then(|t| crate::register::builtin(&t.style_profile))
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

        let (built, report) =
            build::apply(&original, &graph, &translations, &self.profile.branding)?;
        let bytes = built.write()?;
        let validation = validate(
            &original,
            &built,
            &graph,
            &translations,
            self.source_language(),
            &target.language,
        );

        let revision = self.next_build_revision(target)?;
        let dir = self.builds_dir(target).join(format!("{revision:04}"));
        std::fs::create_dir_all(&dir)?;

        let name = self.output_name(target);
        std::fs::write(dir.join(&name), &bytes)?;

        let record = BuildRecord {
            revision,
            language: target.language.clone(),
            profile_revision: self.profile.revision,
            source_sha256: self.profile.source.sha256.clone(),
            translations_applied: translations.len(),
            report,
            validation,
        };
        write_json(&dir.join("build.json"), &record)?;

        // Written under builds/ first and copied to output/ second, so output/ only ever holds a
        // build that finished and has a record.
        std::fs::create_dir_all(self.root.join("output"))?;
        std::fs::write(self.root.join("output").join(&name), &bytes)?;
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
        format!("{}-{}.jar", self.profile.name, target.slug())
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
            object.insert("schemaVersion".into(), serde_json::json!(SCHEMA_VERSION));
        }
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
