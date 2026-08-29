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
use crate::validate::{validate_with_font, ValidationReport};
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
    /// The game's glyph sheet, when it draws its own text (§16).
    ///
    /// Absent means "not established". It does not mean the game uses the device font: a game
    /// that draws from a sheet and has no profile here will show blanks, and saying nothing about
    /// that would be the worst of the three answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<FontProfile>,
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
            font: None,
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

    /// What the game's font can draw, if the project has said which font that is.
    ///
    /// `None` means nobody has established it. That is a different answer from "it covers
    /// everything", and the caller has to keep them apart: a game drawing from a sheet nobody
    /// declared will show blanks, and reporting that as fine is how a localization ships broken.
    pub fn font_coverage(&self) -> crate::Result<Option<Coverage>> {
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
    /// looks them up, which is per-game and belongs to the rule engine (§19) - not built. This
    /// produces the artwork and says so.
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

    /// The per-game patches this project holds (§19).
    pub fn rules(&self) -> crate::Result<Vec<crate::rules::Rule>> {
        crate::rules::load(&self.root)
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

        let mut rule = Rule::new(
            "install-font",
            format!(
                "Replace {} with the sheet holding the Vietnamese letters. The game must also be told the sheet is taller and which character each new cell holds; that part is per-game and is not written here.",
                profile.entry
            ),
        );
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
        Ok(rule)
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

        let (mut built, report) =
            build::apply(&original, &graph, &translations, &self.profile.branding)?;

        // Rules run last, on the archive the text has already been patched into. They are the
        // per-game part of the work - installing a font sheet, changing a layout constant - and
        // running them here means the validation below sees what will actually ship.
        let rules = crate::rules::apply(&self.rules()?, &mut built, &self.root)?;
        let bytes = built.write()?;
        let font = self.font_coverage()?;
        let validation = validate_with_font(
            &original,
            &built,
            &graph,
            &translations,
            self.source_language(),
            &target.language,
            font.as_ref(),
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
            rules,
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
