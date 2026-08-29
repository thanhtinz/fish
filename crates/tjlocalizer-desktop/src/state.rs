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
}

impl ProjectSummary {
    pub fn of(project: &Project) -> Self {
        let profile = project.profile();
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
            source_sha256: profile.source.sha256.clone(),
            revision: profile.revision,
            branding_enabled: profile.branding.enabled,
            needs_analyze: !project.root().join("extracted/capabilities.json").exists(),
            needs_extract: graph.is_none(),
            node_count: nodes,
            translatable_count: translatable,
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
    match context {
        ContextType::Ui => "ui",
        ContextType::Dialogue => "dialogue",
        ContextType::Quest => "quest",
        ContextType::Item => "item",
        ContextType::Skill => "skill",
        ContextType::System => "system",
        ContextType::Tutorial => "tutorial",
        ContextType::Story => "story",
        ContextType::Format => "format",
        ContextType::Technical => "technical",
        ContextType::Unknown => "unknown",
    }
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
