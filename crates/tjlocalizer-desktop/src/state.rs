//! View models handed to the interface.
//!
//! The core's own types are shaped for correctness, not for a table: a `TextNode` knows nothing
//! about its translation, and a translation knows nothing about its node. The interface needs
//! both together, per row, so the joining happens here rather than in TypeScript - keeping the
//! rule about what may be auto-approved in Rust, where the tests are.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tjlocalizer_core::graph::{ContextType, TextNode, TextSource};
use tjlocalizer_core::project::{BuildRecord, Project};
use tjlocalizer_core::suggest::{Candidate, Origin};
use tjlocalizer_core::validate::Severity;

/// Enough about a project to list and reopen it without loading its content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub path: String,
    pub name: String,
    pub target_language: String,
    pub style_profile: String,
    pub source_sha256: String,
    pub revision: u32,
    pub branding_enabled: bool,
    /// False once analysis has been run and the capability manifest exists.
    pub needs_analyze: bool,
    pub needs_extract: bool,
    pub node_count: usize,
    pub translatable_count: usize,
    pub approved_count: usize,
    pub build_count: usize,
}

impl ProjectSummary {
    pub fn of(project: &Project) -> Self {
        let profile = project.profile();
        let graph = project.graph().ok();
        let approved = project.translations().map(|t| t.len()).unwrap_or(0);
        let (nodes, translatable) = match &graph {
            Some(g) => (g.nodes.len(), g.translatable().count()),
            None => (0, 0),
        };
        Self {
            path: project.root().display().to_string(),
            name: profile.name.clone(),
            target_language: profile.localization.target_language.clone(),
            style_profile: profile.localization.style_profile.clone(),
            source_sha256: profile.source.sha256.clone(),
            revision: profile.revision,
            branding_enabled: profile.branding.enabled,
            needs_analyze: !project.root().join("extracted/capabilities.json").exists(),
            needs_extract: graph.is_none(),
            node_count: nodes,
            translatable_count: translatable,
            approved_count: approved,
            build_count: project.builds().map(|b| b.len()).unwrap_or(0),
        }
    }
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
    pub fn of(node: &TextNode, target: Option<&str>, candidate: Option<&Candidate>) -> Self {
        let issues = match target {
            Some(t) => tjlocalizer_core::vietnamese::check(
                &node.source_text,
                t,
                &node.constraints.placeholders,
            )
            .into_iter()
            .map(|i| IssueView {
                blocking: i.code == "placeholder" || i.code == "empty",
                code: i.code,
                detail: i.detail,
            })
            .collect(),
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
