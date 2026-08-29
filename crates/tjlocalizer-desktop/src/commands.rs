//! The commands the interface can call.
//!
//! Every one of them is a thin wrapper over `tjlocalizer_core`. No localization logic lives here
//! and none lives in the frontend: the interface decides what to show, the core decides what is
//! true. That is why "may this candidate be approved without a human?" is answered by
//! `suggest::apply_safe` rather than by a checkbox in TypeScript.
//!
//! Commands that act on one language take a `language` tag. A project can ship in several, and
//! each is a separate body of work.

use crate::csvfmt::{parse_line, quote, BOM};
use crate::state::*;
use std::path::{Path, PathBuf};
use tauri::Manager;
use tjlocalizer_core::build::Branding;
use tjlocalizer_core::lang::{known_languages, Language};
use tjlocalizer_core::project::Project;
use tjlocalizer_core::register;
use tjlocalizer_core::translate::{self, DictionaryProvider, Request};
use tjlocalizer_core::{dictionary_data, suggest};

/// Errors reach the interface as text, because that is all it can do with them: show them. The
/// core's messages are written to be read by a person, so nothing is lost.
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
    targets: Vec<String>,
) -> Reply<ProjectSummary> {
    let jar = Path::new(&jar_path);
    let bytes = std::fs::read(jar).map_err(|e| format!("cannot read {jar_path}: {e}"))?;
    let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
        jar.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "game".to_string())
    });
    let root = Path::new(&into).join(&name);

    let mut project = Project::create(&root, &name, &bytes).map_err(err)?;
    if !targets.is_empty() {
        let chosen = targets
            .iter()
            .map(|tag| {
                let language = Language::new(tag.as_str());
                let style = default_style(&language);
                tjlocalizer_core::project::Target::new(language, style)
            })
            .collect();
        project.profile_mut().targets = chosen;
        project.save().map_err(err)?;
    }

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
    Ok(capability_views(&manifest))
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
    Ok(capability_views(&manifest))
}

fn capability_views(
    manifest: &tjlocalizer_core::detect::CapabilityManifest,
) -> Vec<CapabilityView> {
    manifest
        .capabilities
        .iter()
        .map(|c| CapabilityView {
            id: c.id.clone(),
            confidence: c.confidence,
            evidence: c.evidence.clone(),
        })
        .collect()
}

#[tauri::command]
pub fn extract(path: String) -> Reply<usize> {
    Ok(open(&path)?.extract().map_err(err)?.nodes.len())
}

/// Every node, joined with its approved translation and its current candidate for one language.
#[tauri::command]
pub fn nodes(path: String, language: String) -> Reply<Vec<NodeView>> {
    let project = open(&path)?;
    let language = Language::new(language);
    let graph = project.graph().map_err(err)?;
    let approved = project.translations(&language).map_err(err)?;
    let candidates = project.candidates(&language).map_err(err)?;

    let by_node: std::collections::HashMap<&str, &suggest::Candidate> = candidates
        .candidates
        .iter()
        .map(|c| (c.node_id.as_str(), c))
        .collect();

    let from = project.source_language().clone();
    Ok(graph
        .nodes
        .iter()
        .map(|node| {
            NodeView::of(
                node,
                approved.get(&node.id),
                by_node.get(node.id.as_str()).copied(),
                &from,
                &language,
            )
        })
        .collect())
}

/// What the offline engine makes of one string.
///
/// Computed on demand rather than for every row: it is a starting point a translator asks for,
/// not something to fill a table with.
#[tauri::command]
pub fn gloss(path: String, language: String, node_id: String) -> Reply<Option<GlossView>> {
    let project = open(&path)?;
    let language = Language::new(language);
    let graph = project.graph().map_err(err)?;
    let Some(node) = graph.get(&node_id) else {
        return Ok(None);
    };

    let dictionary = project.dictionary().map_err(err)?;
    let glossary = project.glossary(&language).map_err(err)?;
    let memory = project.memory(&language).map_err(err)?;
    let style = project
        .target(&language)
        .and_then(|t| register::builtin(&t.style_profile));

    let mut provider = DictionaryProvider::new(&dictionary, &glossary);
    if let Some(style) = style.as_ref() {
        provider = provider.with_style(style);
    }

    let request = Request {
        source_text: node.source_text.clone(),
        from: project.source_language().clone(),
        to: language,
        context: format!("{:?}", node.context).to_lowercase(),
        placeholders: node.constraints.placeholders.clone(),
        speaker: Default::default(),
        stance: Default::default(),
    };
    Ok(translate::propose(&request, &memory, &[&provider]).map(|p| GlossView::of(&p)))
}

/// Approves a translation, or clears it when `target` is empty.
///
/// Clearing rather than storing an empty string matters: an empty approved translation would be
/// patched into the game as an empty string, silently blanking the text it replaced.
#[tauri::command]
pub fn set_translation(
    path: String,
    language: String,
    node_id: String,
    target: String,
) -> Reply<()> {
    let project = open(&path)?;
    let language = Language::new(language);
    let mut store = project.translations(&language).map_err(err)?;
    if target.trim().is_empty() {
        store.approved.remove(&node_id);
    } else {
        store.set(&node_id, target);
    }
    project.save_translations(&language, &store).map_err(err)
}

#[tauri::command]
pub fn suggest_all(path: String, language: String, fuzzy_threshold: f32) -> Reply<usize> {
    Ok(open(&path)?
        .suggest(&Language::new(language), fuzzy_threshold)
        .map_err(err)?
        .candidates
        .len())
}

/// Approves only the candidates that restate a decision the project already made.
#[tauri::command]
pub fn apply_safe(path: String, language: String) -> Reply<usize> {
    let project = open(&path)?;
    let language = Language::new(language);
    let set = project.candidates(&language).map_err(err)?;
    let mut approved = project.translations(&language).map_err(err)?;
    let applied = suggest::apply_safe(&set, &mut approved);
    project
        .save_translations(&language, &approved)
        .map_err(err)?;
    Ok(applied)
}

#[tauri::command]
pub fn learn(path: String, language: String) -> Reply<usize> {
    open(&path)?.learn(&Language::new(language)).map_err(err)
}

#[tauri::command]
pub fn build(path: String, language: String) -> Reply<BuildView> {
    Ok(BuildView::of(
        &open(&path)?.build(&Language::new(language)).map_err(err)?,
    ))
}

#[tauri::command]
pub fn build_all(path: String) -> Reply<Vec<BuildView>> {
    Ok(open(&path)?
        .build_all()
        .map_err(err)?
        .iter()
        .map(BuildView::of)
        .collect())
}

#[tauri::command]
pub fn builds(path: String, language: String) -> Reply<Vec<BuildView>> {
    Ok(open(&path)?
        .builds(&Language::new(language))
        .map_err(err)?
        .iter()
        .map(BuildView::of)
        .rev()
        .collect())
}

#[tauri::command]
pub fn rollback(path: String, language: String, revision: u32) -> Reply<BuildView> {
    Ok(BuildView::of(
        &open(&path)?
            .rollback(&Language::new(language), revision)
            .map_err(err)?,
    ))
}

/// Copies a built artifact to wherever the user chose to put it.
///
/// The whole point of a desktop application: the project directory is the tool's business, and
/// where the finished file goes is the user's.
#[tauri::command]
pub fn export_build(
    path: String,
    language: String,
    destination: String,
    overwrite: bool,
) -> Reply<String> {
    let project = open(&path)?;
    let language = Language::new(language);
    let from = project
        .output_path(&language)
        .map_err(err)?
        .ok_or_else(|| format!("{language} has no build yet"))?;

    let mut destination = PathBuf::from(destination);
    // A directory means "put it here under its own name"; anything else is the file name.
    if destination.is_dir() {
        destination = destination.join(from.file_name().expect("a built artifact has a name"));
    }
    if destination.exists() && !overwrite {
        return Err(format!("{} already exists", destination.display()));
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::copy(&from, &destination).map_err(err)?;
    Ok(destination.display().to_string())
}

/// Writes every translatable string to a file a translator can work in outside this application.
///
/// CSV, because that is what opens in every spreadsheet. Quoting is done here rather than by a
/// crate: game text is full of quotes and newlines, and getting it wrong corrupts the file the
/// translator sends back.
#[tauri::command]
pub fn export_translations(
    path: String,
    language: String,
    destination: String,
    only_untranslated: bool,
) -> Reply<usize> {
    let project = open(&path)?;
    let language = Language::new(language);
    let graph = project.graph().map_err(err)?;
    let approved = project.translations(&language).map_err(err)?;

    let mut out = String::from("id,context,location,source,target\n");
    let mut rows = 0usize;
    for node in graph.translatable() {
        let target = approved.get(&node.id).unwrap_or("");
        if only_untranslated && !target.is_empty() {
            continue;
        }
        let location = match &node.source {
            tjlocalizer_core::graph::TextSource::ClassConstant { class, .. } => class.clone(),
            tjlocalizer_core::graph::TextSource::ResourceProperty { resource, key } => {
                format!("{resource}#{key}")
            }
            tjlocalizer_core::graph::TextSource::ResourceLine { resource, line } => {
                format!("{resource}:{}", line + 1)
            }
        };
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            quote(&node.id),
            quote(&format!("{:?}", node.context).to_lowercase()),
            quote(&location),
            quote(&node.source_text),
            quote(target)
        ));
        rows += 1;
    }

    let mut bytes = BOM.to_vec();
    bytes.extend_from_slice(out.as_bytes());
    std::fs::write(&destination, bytes).map_err(err)?;
    Ok(rows)
}

/// Reads back a CSV a translator worked in, and approves what it holds.
///
/// Matched by node id, so a row whose source text was edited still lands in the right place, and
/// a row for a string that no longer exists is reported rather than silently dropped.
#[tauri::command]
pub fn import_translations(path: String, language: String, source: String) -> Reply<ImportReport> {
    let project = open(&path)?;
    let language = Language::new(language);
    let graph = project.graph().map_err(err)?;
    let mut approved = project.translations(&language).map_err(err)?;

    let text = std::fs::read_to_string(&source).map_err(err)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);

    let mut report = ImportReport::default();
    for (number, line) in text.lines().enumerate() {
        if number == 0 || line.trim().is_empty() {
            continue;
        }
        let fields = parse_line(line);
        if fields.len() < 5 {
            report.malformed.push(number + 1);
            continue;
        }
        let (id, target) = (&fields[0], fields[4].trim());
        if graph.get(id).is_none() {
            report.unknown += 1;
            continue;
        }
        if target.is_empty() {
            continue;
        }
        if approved.get(id) == Some(target) {
            report.unchanged += 1;
            continue;
        }
        approved.set(id, target);
        report.applied += 1;
    }
    project
        .save_translations(&language, &approved)
        .map_err(err)?;
    Ok(report)
}

/// Adds a dictionary pack to the project from a file the user chose.
#[tauri::command]
pub fn import_dictionary(path: String, source: String) -> Reply<usize> {
    let text = std::fs::read_to_string(&source).map_err(err)?;
    let pack: tjlocalizer_core::dictionary::Pack =
        serde_json::from_str(&text).map_err(|e| format!("this is not a dictionary pack: {e}"))?;
    let count = pack.entries.len();

    let dir = Path::new(&path).join("dictionary");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let name = format!("{}-{}.json", pack.from.tag(), pack.to.tag());
    std::fs::copy(&source, dir.join(name)).map_err(err)?;
    Ok(count)
}

#[tauri::command]
pub fn add_target(
    path: String,
    language: String,
    style_profile: Option<String>,
) -> Reply<ProjectSummary> {
    let mut project = open(&path)?;
    let language = Language::new(language);
    let style = style_profile.unwrap_or_else(|| default_style(&language));
    project.add_target(language, &style).map_err(err)?;
    Ok(ProjectSummary::of(&project))
}

#[tauri::command]
pub fn remove_target(path: String, language: String) -> Reply<ProjectSummary> {
    let mut project = open(&path)?;
    project
        .remove_target(&Language::new(language))
        .map_err(err)?;
    Ok(ProjectSummary::of(&project))
}

#[tauri::command]
pub fn set_style(path: String, language: String, style_profile: String) -> Reply<ProjectSummary> {
    let mut project = open(&path)?;
    let language = Language::new(language);
    for target in project.profile_mut().targets.iter_mut() {
        if target.language == language {
            target.style_profile = style_profile.clone();
        }
    }
    project.save().map_err(err)?;
    Ok(ProjectSummary::of(&project))
}

#[tauri::command]
pub fn set_source_language(path: String, language: String) -> Reply<ProjectSummary> {
    let mut project = open(&path)?;
    let profile = project.profile_mut();
    profile.source_language.language = Language::new(language);
    profile.source_language.detected = false;
    project.save().map_err(err)?;
    Ok(ProjectSummary::of(&project))
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
pub fn languages() -> Vec<LanguageView> {
    known_languages().iter().map(LanguageView::of).collect()
}

#[tauri::command]
pub fn styles(language: Option<String>) -> Vec<StyleView> {
    let profiles = match language {
        Some(tag) => register::profiles_for(&Language::new(tag)),
        None => register::builtin_profiles(),
    };
    profiles
        .iter()
        .map(|p| {
            let voice = p.voices.first();
            StyleView {
                id: p.id.clone(),
                language: p.language.tag().to_string(),
                description: p.description.clone(),
                first_person: voice
                    .map(|v| v.pronouns.first_singular.clone())
                    .unwrap_or_default(),
                second_person: voice
                    .map(|v| v.pronouns.second_singular.clone())
                    .unwrap_or_default(),
            }
        })
        .collect()
}

#[tauri::command]
pub fn dictionaries(path: Option<String>) -> Reply<Vec<DictionaryView>> {
    let dictionary = match path {
        Some(p) => open(&p)?.dictionary().map_err(err)?,
        None => dictionary_data::builtin(),
    };
    Ok(dictionary
        .directions()
        .iter()
        .map(|(from, to)| DictionaryView {
            entries: dictionary
                .packs
                .iter()
                .filter(|p| p.from == *from && p.to == *to)
                .map(|p| p.entries.len())
                .sum(),
            from_name: from.display_name(),
            to_name: to.display_name(),
            from: from.tag().to_string(),
            to: to.tag().to_string(),
        })
        .collect())
}

/// The register to start a language with.
fn default_style(language: &Language) -> String {
    register::profiles_for(language)
        .first()
        .map(|p| p.id.clone())
        .unwrap_or_else(|| format!("{}-plain", language.base()))
}
