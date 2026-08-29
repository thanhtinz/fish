//! View models handed to the interface.
//!
//! The core's own types are shaped for correctness, not for a table: a `TextNode` knows nothing
//! about its translation, and a translation knows nothing about its node. The interface needs
//! both together, per row, so the joining happens here rather than in TypeScript - keeping the
//! rule about what may be auto-approved in Rust, where the tests are.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tjlocalizer_core::graph::{ContextType, TextNode, TextSource};
use tjlocalizer_core::lang::Language;
use tjlocalizer_core::project::{BuildRecord, Project};
use tjlocalizer_core::suggest::{Candidate, Origin};
use tjlocalizer_core::translate::{Completeness, Proposal};
use tjlocalizer_core::validate::Severity;

/// One target language of a project, with its own progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetView {
    pub tag: String,
    pub name: String,
    pub style_profile: String,
    pub enabled: bool,
    pub approved_count: usize,
    pub build_count: usize,
    /// Where the last build was published, if it is still there.
    pub output_path: Option<String>,
}

/// One row of the recent list: a project, or the reason it will not open.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentView {
    pub path: String,
    pub summary: Option<ProjectSummary>,
    pub error: Option<String>,
}

/// Enough about a project to list and reopen it without loading its content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub path: String,
    pub name: String,
    pub source_language: String,
    pub source_language_name: String,
    pub source_language_detected: bool,
    pub targets: Vec<TargetView>,
    pub source_sha256: String,
    pub revision: u32,
    pub branding_enabled: bool,
    /// False once analysis has been run and the capability manifest exists.
    pub needs_analyze: bool,
    pub needs_extract: bool,
    pub node_count: usize,
    pub translatable_count: usize,
    /// What kind of package this is, and what cannot be done with it.
    ///
    /// Carried on the summary rather than fetched separately because it changes what the
    /// interface may promise: an Android package can be translated and cannot be handed to
    /// somebody as an installable file, and a person should learn that when they open the
    /// project rather than when the build finishes.
    pub package: PackageView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageView {
    pub label: String,
    pub can_repackage: bool,
    pub note: Option<String>,
    pub evidence: Vec<String>,
    pub readable: Vec<ReadableView>,
    pub opaque: Vec<OpaqueView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadableView {
    pub entry: String,
    pub format: String,
    pub fields: usize,
    /// Whether a build can write this file back. Readable and writable are different facts.
    pub writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpaqueView {
    pub entry: String,
    pub reason: String,
}

impl Default for PackageView {
    /// What to say about a package that could not be read at all.
    fn default() -> Self {
        PackageView {
            label: "unknown".into(),
            can_repackage: false,
            note: None,
            evidence: Vec::new(),
            readable: Vec::new(),
            opaque: Vec::new(),
        }
    }
}

impl ProjectSummary {
    pub fn of(project: &Project) -> Self {
        let profile = project.profile();
        let package = project
            .package()
            .map(|found| PackageView {
                label: found.kind.label().to_string(),
                can_repackage: found.kind.can_repackage(),
                note: found.kind.repackaging_note().map(str::to_string),
                evidence: found.evidence,
                readable: found
                    .readable
                    .into_iter()
                    .map(|r| ReadableView {
                        entry: r.entry,
                        format: r.format,
                        fields: r.fields,
                        writable: r.writable,
                    })
                    .collect(),
                opaque: found
                    .opaque
                    .into_iter()
                    .map(|o| OpaqueView {
                        entry: o.entry,
                        reason: o.reason,
                    })
                    .collect(),
            })
            .unwrap_or_default();
        let graph = project.graph().ok();
        let (nodes, translatable) = match &graph {
            Some(g) => (g.nodes.len(), g.translatable().count()),
            None => (0, 0),
        };
        let targets = profile
            .targets
            .iter()
            .map(|t| TargetView {
                tag: t.language.tag().to_string(),
                name: t.language.display_name(),
                style_profile: t.style_profile.clone(),
                enabled: t.enabled,
                approved_count: project
                    .translations(&t.language)
                    .map(|s| s.len())
                    .unwrap_or(0),
                build_count: project.builds(&t.language).map(|b| b.len()).unwrap_or(0),
                output_path: project
                    .output_path(&t.language)
                    .ok()
                    .flatten()
                    .map(|p| p.display().to_string()),
            })
            .collect();

        Self {
            path: project.root().display().to_string(),
            name: profile.name.clone(),
            source_language: profile.source_language.language.tag().to_string(),
            source_language_name: profile.source_language.language.display_name(),
            source_language_detected: profile.source_language.detected,
            targets,
            source_sha256: profile.source.sha256().to_string(),
            revision: profile.revision,
            branding_enabled: profile.branding.enabled,
            needs_analyze: !project.root().join("extracted/capabilities.json").exists(),
            needs_extract: graph.is_none(),
            node_count: nodes,
            translatable_count: translatable,
            package,
        }
    }
}

/// A language the interface can offer, whether or not the project uses it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageView {
    pub tag: String,
    pub name: String,
    pub script: String,
}

impl LanguageView {
    pub fn of(language: &Language) -> Self {
        Self {
            tag: language.tag().to_string(),
            name: language.display_name(),
            script: format!("{:?}", language.script()).to_lowercase(),
        }
    }
}

/// A register profile, for the picker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleView {
    pub id: String,
    pub language: String,
    pub description: String,
    pub first_person: String,
    pub second_person: String,
}

/// What the offline engine makes of a string, for the detail panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossView {
    pub text: String,
    pub completeness: String,
    pub confidence: f32,
    pub engine: String,
    pub terms: Vec<GlossTerm>,
    pub unresolved: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossTerm {
    pub source: String,
    pub target: String,
    pub domain: String,
}

impl GlossView {
    pub fn of(proposal: &Proposal) -> Self {
        Self {
            text: proposal.target_text.clone(),
            completeness: match proposal.completeness {
                Completeness::Complete => "complete",
                Completeness::Partial => "partial",
                Completeness::None => "none",
            }
            .into(),
            confidence: proposal.confidence,
            engine: proposal.engine.clone(),
            terms: proposal
                .terms
                .iter()
                .map(|t| GlossTerm {
                    source: t.source.clone(),
                    target: t.target.clone(),
                    domain: format!("{:?}", t.domain).to_lowercase(),
                })
                .collect(),
            unresolved: proposal.unresolved.clone(),
            notes: proposal.notes.clone(),
        }
    }
}

/// The external engine's settings, as the interface sees them.
///
/// The key is never in here. What the interface needs to know is whether one is stored, not what
/// it is - and a view model is the thing most likely to end up in a log or an error report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineView {
    pub configured: bool,
    pub enabled: bool,
    pub kind: String,
    pub endpoint: String,
    pub model: Option<String>,
    pub has_key: bool,
    /// The families this build can talk to, for the picker.
    pub kinds: Vec<EngineKindView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineKindView {
    pub id: String,
    pub default_endpoint: String,
    /// Whether this family can be told the register and terminology in words. The others are
    /// only checked on the way back, which the interface should say.
    pub takes_instructions: bool,
}

/// How the analysis side is set up, for the settings screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalystView {
    pub enabled: bool,
    pub endpoint: String,
    pub model: String,
    pub has_key: bool,
    /// The models offered. The list is short and named rather than free text, because a mistyped
    /// model is a failed call with an unhelpful message.
    pub models: Vec<String>,
}

/// What a scan would send, shown before it is sent.
///
/// The file names, because that is what a person is consenting to - and the count of tokens, from
/// the service rather than from characters divided by four.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPreview {
    pub paths: Vec<String>,
    pub model: String,
    /// None when the count could not be got. Shown as unknown rather than as a guess.
    pub tokens: Option<u64>,
    /// Why the count is missing, when it is.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub trouble: String,
}

/// One suggestion, and the fact that it is one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionView {
    pub path: String,
    pub why: String,
    pub confidence: f32,
    /// The model that said so, so a stale suggestion can be told from a fresh one.
    pub model: String,
}

/// What importing a game directory found, and what it passed over.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestView {
    pub project: ProjectSummary,
    /// How many files the game holds in total. Shown first, because "23 files" on its own reads
    /// like something went wrong and "41 812 files, 23 read" reads like the tool did its job.
    pub scanned: usize,
    pub total_size: u64,
    pub read: usize,
    pub read_size: u64,
    /// What the game looks like it was made with, from names alone.
    pub evidence: Vec<String>,
    pub skipped: Vec<SkippedView>,
}

/// A file that was found and not read, with the reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedView {
    pub path: String,
    pub size: u64,
    pub reason: String,
}

/// A direction the dictionary covers, for the settings screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryView {
    pub from: String,
    pub to: String,
    pub from_name: String,
    pub to_name: String,
    pub entries: usize,
}

/// Where a string lives, in a form a person can read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    /// "class" or "resource" - the interface groups by this.
    pub kind: String,
    /// The file the string came from.
    pub file: String,
    /// A property key, a line number, or a constant pool index.
    pub detail: String,
}

impl Location {
    fn of(source: &TextSource) -> Self {
        match source {
            TextSource::ClassConstant {
                class, utf8_index, ..
            } => Location {
                kind: "class".into(),
                file: class.clone(),
                detail: format!("constant #{utf8_index}"),
            },
            TextSource::ResourceProperty { resource, key } => Location {
                kind: "resource".into(),
                file: resource.clone(),
                detail: key.clone(),
            },
            TextSource::ResourceLine { resource, line } => Location {
                kind: "resource".into(),
                file: resource.clone(),
                detail: format!("line {}", line + 1),
            },
        }
    }
}

/// A proposal shown beside a row, with why it is proposed and whether it may be taken as read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateView {
    pub target: String,
    /// "memory", "memory-fuzzy" or "glossary" - the interface colours these differently, because
    /// a fuzzy proposal deserves a different amount of attention from an exact one.
    pub origin: String,
    pub score: Option<f32>,
    pub auto_approvable: bool,
}

impl CandidateView {
    fn of(candidate: &Candidate) -> Self {
        let (origin, score) = match candidate.origin {
            Origin::MemoryExact => ("memory", None),
            Origin::MemoryFuzzy { score } => ("memory-fuzzy", Some(score)),
            Origin::GlossaryTerm => ("glossary", None),
        };
        Self {
            target: candidate.target.clone(),
            origin: origin.into(),
            score,
            auto_approvable: candidate.auto_approvable,
        }
    }
}

/// One row of the translation table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeView {
    pub id: String,
    pub source: String,
    pub target: Option<String>,
    pub context: String,
    pub translatable: bool,
    pub location: Location,
    pub placeholders: Vec<String>,
    pub source_encoding: Option<String>,
    pub candidate: Option<CandidateView>,
    /// Quality problems in the current translation, if there is one. Recomputed on every read so
    /// the interface can never show a stale green row.
    pub issues: Vec<IssueView>,
    /// How wide the original and the translation draw, in the game's own pixels.
    ///
    /// Only set when the game draws from a proportional sheet. On a fixed-pitch one this is the
    /// character count in other units, and showing it would suggest the tool knows something it
    /// does not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_width: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueView {
    pub code: String,
    pub detail: String,
    pub blocking: bool,
}

impl NodeView {
    /// Joins a node with its translation and candidate.
    ///
    /// The languages are needed because the quality checks are language-specific: a length that
    /// is suspicious in Vietnamese is normal in Chinese, and a check that fires on everything is
    /// one a translator learns to ignore.
    pub fn of(
        node: &TextNode,
        target: Option<&str>,
        candidate: Option<&Candidate>,
        from: &Language,
        to: &Language,
    ) -> Self {
        Self::measured(node, target, candidate, from, to, None)
    }

    /// The same, plus what the two strings measure in the game's own font (§24).
    pub fn measured(
        node: &TextNode,
        target: Option<&str>,
        candidate: Option<&Candidate>,
        from: &Language,
        to: &Language,
        metrics: Option<&tjlocalizer_core::font::metrics::Metrics>,
    ) -> Self {
        let metrics = metrics.filter(|m| !m.monospaced);
        let (source_width, target_width) = match metrics {
            Some(m) => (
                m.measure(&node.source_text),
                target.and_then(|t| m.measure(t)),
            ),
            None => (None, None),
        };
        let issues = match target {
            Some(t) => {
                let mut found = tjlocalizer_core::quality::check(
                    &node.source_text,
                    t,
                    &node.constraints.placeholders,
                    from,
                    to,
                );
                if to.base() == "vi" {
                    found.extend(tjlocalizer_core::vietnamese::check(t));
                }
                found
                    .into_iter()
                    .map(|i| IssueView {
                        blocking: i.code == "placeholder" || i.code == "empty",
                        code: i.code,
                        detail: i.detail,
                    })
                    .collect()
            }
            None => Vec::new(),
        };
        Self {
            source_width,
            target_width,
            id: node.id.clone(),
            source: node.source_text.clone(),
            target: target.map(str::to_string),
            context: context_label(node.context).to_string(),
            translatable: node.context.is_translatable(),
            location: Location::of(&node.source),
            placeholders: node.constraints.placeholders.clone(),
            source_encoding: node.source_encoding.clone(),
            candidate: candidate.map(CandidateView::of),
            issues,
        }
    }
}

fn context_label(context: ContextType) -> &'static str {
    // The core's own name for it. A second spelling here would drift from the one the dictionary
    // scores entries against, and a whole domain would quietly stop matching.
    context.key()
}

/// A build, flattened for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildView {
    pub revision: u32,
    pub language: String,
    pub profile_revision: u32,
    pub literals_patched: usize,
    pub resources_patched: usize,
    pub translations_applied: usize,
    pub output_sha256: String,
    pub ok: bool,
    pub findings: Vec<FindingView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingView {
    pub severity: String,
    pub check: String,
    pub detail: String,
}

impl BuildView {
    pub fn of(record: &BuildRecord) -> Self {
        Self {
            revision: record.revision,
            language: record.language.tag().to_string(),
            profile_revision: record.profile_revision,
            literals_patched: record.report.literals_patched,
            resources_patched: record.report.resources_patched,
            translations_applied: record.translations_applied,
            output_sha256: record.report.output_sha256.clone(),
            ok: record.validation.is_ok(),
            findings: record
                .validation
                .findings
                .iter()
                .map(|f| FindingView {
                    severity: match f.severity {
                        Severity::Error => "error".into(),
                        Severity::Warning => "warning".into(),
                    },
                    check: f.check.clone(),
                    detail: f.detail.clone(),
                })
                .collect(),
        }
    }
}

/// One detected capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityView {
    pub id: String,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

/// Exactly what a request to the engine would contain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnginePreview {
    pub url: String,
    pub instructions: String,
    pub body: String,
}

/// What came back from a CSV a translator returned.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub applied: usize,
    pub unchanged: usize,
    /// Rows for strings this project no longer has - the game was re-imported and changed.
    pub unknown: usize,
    /// Line numbers that did not parse, so they can be looked at rather than guessed about.
    pub malformed: Vec<usize>,
}

/// The list of projects the user has opened, kept beside the application's own config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Recents {
    pub paths: Vec<String>,
}

impl Recents {
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(Self::file(dir))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn remember(&mut self, dir: &Path, path: &str) {
        self.paths.retain(|p| p != path);
        self.paths.insert(0, path.to_string());
        self.paths.truncate(12);
        let _ = std::fs::create_dir_all(dir);
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::file(dir), text);
        }
    }

    /// Drops entries whose directory is gone, so the list cannot fill with dead rows.
    pub fn existing(&self) -> Vec<String> {
        self.paths
            .iter()
            .filter(|p| Path::new(p).join("project.json").exists())
            .cloned()
            .collect()
    }

    fn file(dir: &Path) -> PathBuf {
        dir.join("recent-projects.json")
    }
}

/// The game's font, as the interface sees it.
///
/// `declared` is kept apart from "covers everything" on purpose. A project where nobody has said
/// which image the font is has an unknown answer, not a good one, and an interface that shows the
/// two the same way is how a localization ships with blank boxes where the accents should be.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontView {
    pub declared: bool,
    /// The archive entry holding the sheet, empty when the game uses the device font.
    pub entry: String,
    pub device_font: bool,
    pub grid: Option<GridView>,
    /// The characters the sheet lays out. Empty means printable ASCII.
    pub order: String,
    pub mark_library: Option<String>,
    pub marks_from: Option<String>,
    /// How many of the 134 Vietnamese letters the font already draws.
    pub covered: usize,
    pub required: usize,
    /// Those it cannot draw, so the interface can show them rather than a number.
    pub missing: String,
    /// Of the missing ones, how many can be built from letters the sheet already has.
    pub composable: usize,
    /// Set when the font is declared but cannot be read - a wrong entry, a missing grid.
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridView {
    pub cell_width: u32,
    pub cell_height: u32,
    pub columns: u32,
    pub rows: u32,
}

impl From<tjlocalizer_core::font::sheet::Grid> for GridView {
    fn from(g: tjlocalizer_core::font::sheet::Grid) -> Self {
        GridView {
            cell_width: g.cell_width,
            cell_height: g.cell_height,
            columns: g.columns,
            rows: g.rows,
        }
    }
}

impl From<GridView> for tjlocalizer_core::font::sheet::Grid {
    fn from(g: GridView) -> Self {
        tjlocalizer_core::font::sheet::Grid {
            cell_width: g.cell_width,
            cell_height: g.cell_height,
            columns: g.columns,
            rows: g.rows,
        }
    }
}

/// One image in the archive that could be the font, with the grids that would fit it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetCandidateView {
    pub entry: String,
    pub width: u32,
    pub height: u32,
    pub ink_share: f32,
    pub colours: usize,
    pub grids: Vec<GridSuggestionView>,
    /// The image itself, so a person can look at it instead of guessing from numbers.
    pub image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridSuggestionView {
    #[serde(flatten)]
    pub grid: GridView,
    pub fit: f32,
    /// How many cells this grid gives, against the 95 printable ASCII a sheet usually holds.
    pub capacity: u32,
}

/// One font in the chosen folder, measured against this game's sheet.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFitView {
    pub path: String,
    pub name: String,
    /// Letters whose marks came from this typeface rather than being drawn.
    pub from_typeface: usize,
    pub composed: usize,
    pub share: f32,
    pub chosen: bool,
}

/// What composing produced, and what it did not.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionView {
    pub path: String,
    pub added: String,
    /// Letters left out, grouped by why.
    ///
    /// Grouped because the reasons repeat: a sheet with no headroom refuses the same way for
    /// sixty letters, and sixty copies of one sentence is a wall of text nobody reads - which
    /// hides the one line that says what to do about it.
    pub skipped: Vec<SkippedGroupView>,
    pub from_typeface: usize,
    pub typeface: Option<String>,
    pub image: String,
}

/// What a folder of fonts turned out to hold, and how much of it was actually measured.
///
/// The three counts are kept apart because they answer different questions: how many fonts are
/// in the folder, how many could supply every Vietnamese mark, and how many were tried against
/// this sheet. Reporting only the last would make a sampled folder look like an exhausted one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FontScan {
    pub found: usize,
    pub covering: usize,
    pub measured: usize,
    pub fonts: Vec<FontFitView>,
}

/// One reason letters were left out, and which letters those were.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedGroupView {
    pub reason: String,
    pub letters: String,
}

/// One per-game patch, as the interface sees it (§19).
///
/// `ready` is computed in Rust rather than by the interface checking three fields, because it is
/// the answer to "will this run?" and a display that got it wrong would tell somebody their font
/// was installed when it was not.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleView {
    pub id: String,
    pub description: String,
    pub enabled: bool,
    pub ready: bool,
    /// What it would change, in numbers read from this game.
    pub effects: Vec<String>,
    /// Why it cannot run. Empty when it fits.
    pub unmet: Vec<String>,
}

impl From<tjlocalizer_core::rules::RulePlan> for RuleView {
    fn from(plan: tjlocalizer_core::rules::RulePlan) -> Self {
        RuleView {
            ready: plan.ready(),
            id: plan.id,
            description: plan.description,
            enabled: plan.enabled,
            effects: plan.effects,
            unmet: plan.unmet,
        }
    }
}

/// A shorter way of saying the same thing, and why it is being offered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlternativeView {
    pub text: String,
    pub width: Option<u32>,
    pub why: String,
}

impl From<tjlocalizer_core::shorten::Alternative> for AlternativeView {
    fn from(a: tjlocalizer_core::shorten::Alternative) -> Self {
        AlternativeView {
            text: a.text,
            width: a.width,
            why: a.why,
        }
    }
}

/// One image in the game, with what its shape suggests and what a person decided about it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAssetView {
    pub entry: String,
    pub width: u32,
    pub height: u32,
    pub colours: usize,
    pub hints: Vec<tjlocalizer_core::assets::Hint>,
    /// The image itself, so the question "does this have words on it" can be answered by looking.
    pub image: String,
    /// Set once somebody has said this carries words.
    pub says: Option<String>,
    pub replacement: Option<String>,
    pub marked: bool,
}
