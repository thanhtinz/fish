//! The commands the interface can call.
//!
//! Every one of them is a thin wrapper over `tjlocalizer_core`. No localization logic lives here
//! and none lives in the frontend: the interface decides what to show, the core decides what is
//! true. That is why, for instance, "may this candidate be approved without a human?" is answered
//! by `suggest::apply_safe` rather than by a checkbox in TypeScript.

use crate::state::*;
use std::path::{Path, PathBuf};
use tauri::Manager;
use tjlocalizer_core::build::Branding;
use tjlocalizer_core::project::Project;
use tjlocalizer_core::suggest;

/// Errors reach the interface as text, because that is all it can do with them: show them.
/// The core's messages are written to be read by a person, so nothing is lost.
type Reply<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn open(path: &str) -> Reply<Project> {
    Project::open(path).map_err(err)
}

fn config_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[tauri::command]
pub fn recent_projects(app: tauri::AppHandle) -> Vec<ProjectSummary> {
    Recents::load(&config_dir(&app))
        .existing()
        .iter()
        .filter_map(|p| Project::open(p).ok())
        .map(|p| ProjectSummary::of(&p))
        .collect()
}

/// Imports a JAR into a new project directory.
///
/// `into` is the parent directory the user picked; the project gets its own folder inside it, so
/// importing two games into the same place cannot have them overwrite each other.
#[tauri::command]
pub fn import_jar(
    app: tauri::AppHandle,
    jar_path: String,
    into: String,
    name: Option<String>,
) -> Reply<ProjectSummary> {
    let jar = Path::new(&jar_path);
    let bytes = std::fs::read(jar).map_err(|e| format!("cannot read {jar_path}: {e}"))?;
    let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
        jar.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "game".to_string())
    });
    let root = Path::new(&into).join(&name);

    let project = Project::create(&root, &name, &bytes).map_err(err)?;
    let summary = ProjectSummary::of(&project);
    Recents::load(&config_dir(&app)).remember(&config_dir(&app), &summary.path);
    Ok(summary)
}

#[tauri::command]
pub fn open_project(app: tauri::AppHandle, path: String) -> Reply<ProjectSummary> {
    let project = open(&path)?;
    let summary = ProjectSummary::of(&project);
    Recents::load(&config_dir(&app)).remember(&config_dir(&app), &summary.path);
    Ok(summary)
}

#[tauri::command]
pub fn project_summary(path: String) -> Reply<ProjectSummary> {
    Ok(ProjectSummary::of(&open(&path)?))
}

#[tauri::command]
pub fn analyze(path: String) -> Reply<Vec<CapabilityView>> {
    let manifest = open(&path)?.analyze().map_err(err)?;
    Ok(manifest
        .capabilities
        .iter()
        .map(|c| CapabilityView {
            id: c.id.clone(),
            confidence: c.confidence,
            evidence: c.evidence.clone(),
        })
        .collect())
}

#[tauri::command]
pub fn capabilities(path: String) -> Reply<Vec<CapabilityView>> {
    let file = Path::new(&path).join("extracted/capabilities.json");
    if !file.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(file).map_err(err)?;
    let manifest: tjlocalizer_core::detect::CapabilityManifest =
        serde_json::from_str(&text).map_err(err)?;
    Ok(manifest
        .capabilities
        .iter()
        .map(|c| CapabilityView {
            id: c.id.clone(),
            confidence: c.confidence,
            evidence: c.evidence.clone(),
        })
        .collect())
}

#[tauri::command]
pub fn extract(path: String) -> Reply<usize> {
    Ok(open(&path)?.extract().map_err(err)?.nodes.len())
}

/// Every node, joined with its approved translation and its current candidate.
#[tauri::command]
pub fn nodes(path: String) -> Reply<Vec<NodeView>> {
    let project = open(&path)?;
    let graph = project.graph().map_err(err)?;
    let approved = project.translations().map_err(err)?;
    let candidates = project.candidates().map_err(err)?;

    let by_node: std::collections::HashMap<&str, &suggest::Candidate> = candidates
        .candidates
        .iter()
        .map(|c| (c.node_id.as_str(), c))
        .collect();

    Ok(graph
        .nodes
        .iter()
        .map(|node| {
            NodeView::of(
                node,
                approved.get(&node.id),
                by_node.get(node.id.as_str()).copied(),
            )
        })
        .collect())
}

/// Approves a translation, or clears it when `target` is empty.
///
/// Clearing rather than storing an empty string matters: an empty approved translation would be
/// patched into the game as an empty string, silently blanking the text it replaced.
#[tauri::command]
pub fn set_translation(path: String, node_id: String, target: String) -> Reply<()> {
    let project = open(&path)?;
    let mut store = project.translations().map_err(err)?;
    if target.trim().is_empty() {
        store.approved.remove(&node_id);
    } else {
        store.set(&node_id, target);
    }
    project.save_translations(&store).map_err(err)
}

#[tauri::command]
pub fn suggest_all(path: String, fuzzy_threshold: f32) -> Reply<usize> {
    Ok(open(&path)?
        .suggest(fuzzy_threshold)
        .map_err(err)?
        .candidates
        .len())
}

/// Approves only the candidates that restate a decision the project already made.
#[tauri::command]
pub fn apply_safe(path: String) -> Reply<usize> {
    let project = open(&path)?;
    let set = project.candidates().map_err(err)?;
    let mut approved = project.translations().map_err(err)?;
    let applied = suggest::apply_safe(&set, &mut approved);
    project.save_translations(&approved).map_err(err)?;
    Ok(applied)
}

#[tauri::command]
pub fn learn(path: String) -> Reply<usize> {
    open(&path)?.learn().map_err(err)
}

#[tauri::command]
pub fn build(path: String) -> Reply<BuildView> {
    Ok(BuildView::of(&open(&path)?.build().map_err(err)?))
}

#[tauri::command]
pub fn builds(path: String) -> Reply<Vec<BuildView>> {
    Ok(open(&path)?
        .builds()
        .map_err(err)?
        .iter()
        .map(BuildView::of)
        .rev()
        .collect())
}

#[tauri::command]
pub fn rollback(path: String, revision: u32) -> Reply<BuildView> {
    Ok(BuildView::of(
        &open(&path)?.rollback(revision).map_err(err)?,
    ))
}

#[tauri::command]
pub fn set_branding(path: String, enabled: bool) -> Reply<ProjectSummary> {
    let mut project = open(&path)?;
    project.profile_mut().branding = Branding {
        enabled,
        ..Branding::default()
    };
    project.save().map_err(err)?;
    Ok(ProjectSummary::of(&project))
}

#[tauri::command]
pub fn set_localization(
    path: String,
    target_language: String,
    style_profile: String,
) -> Reply<ProjectSummary> {
    let mut project = open(&path)?;
    project.profile_mut().localization.target_language = target_language;
    project.profile_mut().localization.style_profile = style_profile;
    project.save().map_err(err)?;
    Ok(ProjectSummary::of(&project))
}

/// The path of the artifact the last build published, for "show me the file".
#[tauri::command]
pub fn output_path(path: String) -> Reply<Option<String>> {
    let project = open(&path)?;
    let file = Path::new(&path).join("output").join(project.output_name());
    Ok(file.exists().then(|| file.display().to_string()))
}
