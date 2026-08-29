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
use tjlocalizer_core::font::library;
use tjlocalizer_core::font::outline::MarkSource;
use tjlocalizer_core::lang::{known_languages, Language};
use tjlocalizer_core::project::{FontProfile, Project};
use tjlocalizer_core::provider::{Briefing, HttpProvider, ProviderConfig, ProviderKind};
use tjlocalizer_core::register;
use tjlocalizer_core::secrets::Keys;
use tjlocalizer_core::translate::Provider as _;
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

/// The projects the user has opened, including any that will not open.
///
/// A project that fails to load is listed with its reason rather than dropped. Dropping it is
/// what a `filter_map` does, and the result is that a project quietly disappears from the list
/// and the user concludes their work is gone - which is exactly what happened here once, when a
/// settings field changed shape.
#[tauri::command]
pub fn recent_projects(app: tauri::AppHandle) -> Vec<RecentView> {
    Recents::load(&config_dir(&app))
        .existing()
        .iter()
        .map(|path| match Project::open(path) {
            Ok(project) => RecentView {
                path: path.clone(),
                summary: Some(ProjectSummary::of(&project)),
                error: None,
            },
            Err(e) => RecentView {
                path: path.clone(),
                summary: None,
                error: Some(e.to_string()),
            },
        })
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

#[tauri::command]
pub fn engine(app: tauri::AppHandle, path: String) -> Reply<EngineView> {
    let project = open(&path)?;
    let config = project.profile().provider.clone();
    let keys = Keys::load(&config_dir(&app));
    let current = config.clone().unwrap_or_default();
    Ok(EngineView {
        configured: config.is_some(),
        enabled: current.enabled,
        kind: current.kind.id().to_string(),
        endpoint: current.endpoint.clone(),
        model: current.model.clone(),
        has_key: keys.has(&current.endpoint),
        kinds: ProviderKind::all()
            .iter()
            .map(|k| EngineKindView {
                id: k.id().to_string(),
                default_endpoint: k.default_endpoint().to_string(),
                takes_instructions: k.takes_instructions(),
            })
            .collect(),
    })
}

#[tauri::command]
pub fn set_engine(
    app: tauri::AppHandle,
    path: String,
    kind: String,
    endpoint: String,
    model: Option<String>,
    enabled: bool,
) -> Reply<EngineView> {
    let mut project = open(&path)?;
    let kind = ProviderKind::all()
        .into_iter()
        .find(|k| k.id() == kind)
        .ok_or_else(|| format!("unknown engine {kind:?}"))?;
    project.profile_mut().provider = Some(ProviderConfig {
        enabled,
        kind,
        endpoint,
        model: model.filter(|m| !m.trim().is_empty()),
        timeout_seconds: 30,
    });
    project.save().map_err(err)?;
    engine(app, path)
}

/// Stores or clears the key for the configured endpoint.
///
/// Keyed by endpoint and kept outside the project, because a project is a folder people commit
/// and send to translators.
#[tauri::command]
pub fn set_engine_key(app: tauri::AppHandle, path: String, key: String) -> Reply<EngineView> {
    let project = open(&path)?;
    let endpoint = project
        .profile()
        .provider
        .as_ref()
        .map(|p| p.endpoint.clone())
        .ok_or("configure an engine first")?;
    let dir = config_dir(&app);
    let mut keys = Keys::load(&dir);
    keys.set(&endpoint, &key);
    keys.save(&dir).map_err(err)?;
    engine(app, path)
}

/// The exact request that would go out for a string, without sending it.
///
/// The user is about to send their game's text to a third party. Being able to see precisely what
/// would go is the difference between a decision and a leap.
#[tauri::command]
pub fn engine_preview(path: String, language: String, text: String) -> Reply<EnginePreview> {
    let project = open(&path)?;
    let language = Language::new(language);
    let config = project
        .profile()
        .provider
        .clone()
        .ok_or("no engine is configured")?;
    let glossary = project.glossary(&language).map_err(err)?;
    let style = project.style(&language);

    let provider = HttpProvider::new(
        config,
        "<your key>".to_string(),
        Briefing {
            glossary: &glossary,
            style: style.as_ref(),
        },
    );
    let request = Request {
        source_text: text.clone(),
        from: project.source_language().clone(),
        to: language,
        context: "ui".into(),
        placeholders: tjlocalizer_core::graph::find_placeholders(&text),
        speaker: Default::default(),
        stance: Default::default(),
    };
    let call = provider.build_call(&request);
    Ok(EnginePreview {
        url: call.url,
        instructions: provider.instructions(&request),
        body: call.body,
    })
}

/// Asks the configured engine about one string.
///
/// One at a time and only when asked: nothing reaches the network as a side effect of opening a
/// row or running the offline pipeline.
#[tauri::command]
pub fn engine_translate(
    app: tauri::AppHandle,
    path: String,
    language: String,
    node_id: String,
) -> Reply<Option<GlossView>> {
    let project = open(&path)?;
    let language = Language::new(language);
    let config = project
        .profile()
        .provider
        .clone()
        .ok_or("no engine is configured")?;
    if !config.enabled {
        return Err("the engine is switched off".into());
    }
    let key = Keys::load(&config_dir(&app))
        .get(&config.endpoint)
        .map(str::to_string)
        .ok_or("no key stored for that endpoint")?;

    let graph = project.graph().map_err(err)?;
    let Some(node) = graph.get(&node_id) else {
        return Ok(None);
    };
    let glossary = project.glossary(&language).map_err(err)?;
    let style = project.style(&language);

    let provider = HttpProvider::new(
        config,
        key,
        Briefing {
            glossary: &glossary,
            style: style.as_ref(),
        },
    );
    let request = Request {
        source_text: node.source_text.clone(),
        from: project.source_language().clone(),
        to: language,
        context: format!("{:?}", node.context).to_lowercase(),
        placeholders: node.constraints.placeholders.clone(),
        speaker: Default::default(),
        stance: Default::default(),
    };
    Ok(provider.propose(&request).map(|p| GlossView::of(&p)))
}

/// The register to start a language with.
fn default_style(language: &Language) -> String {
    register::profiles_for(language)
        .first()
        .map(|p| p.id.clone())
        .unwrap_or_else(|| format!("{}-plain", language.base()))
}

// ---------------------------------------------------------------------------------------------
// The game's font.
//
// A J2ME game usually draws from a strip of pixels rather than a system font, and that strip
// holds the letters the game was written for - which for a game from China or Japan means ASCII
// and nothing else. Vietnamese needs 134 letters beyond ASCII, so without doing something about
// the font a finished translation renders as blanks. These commands are how a person does that
// something from the application instead of the command line.
// ---------------------------------------------------------------------------------------------

/// A PNG the interface can show, as a data URI.
///
/// Tauri will not load an arbitrary path from the filesystem, and the images here live in the
/// project directory and inside the user's archive. Inlining them avoids granting the webview a
/// read of the disk in order to show a picture of a font.
fn data_uri(png: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::from("data:image/png;base64,");
    for chunk in png.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - i * 6) & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn font_view(project: &Project) -> FontView {
    let Some(profile) = project.profile().font.clone() else {
        return FontView {
            required: tjlocalizer_core::font::vietnamese_required().len(),
            ..Default::default()
        };
    };

    let mut view = FontView {
        declared: true,
        entry: profile.entry.clone(),
        device_font: profile.device_font,
        grid: profile.grid.map(GridView::from),
        order: profile.order.clone(),
        mark_library: profile.mark_library.map(|p| p.display().to_string()),
        marks_from: profile.marks_from.map(|p| p.display().to_string()),
        required: tjlocalizer_core::font::vietnamese_required().len(),
        ..Default::default()
    };

    // A declared font that cannot be read is reported as a problem rather than as zero coverage.
    // Zero coverage is a fact about a font; this is a fact about the project settings, and the
    // two need different fixes.
    match project.font_coverage() {
        Ok(Some(coverage)) => {
            let missing = coverage.missing_for_vietnamese();
            view.covered = view.required - missing.len();
            view.composable = coverage.composable().len();
            view.missing = missing.into_iter().collect();
        }
        Ok(None) => view.declared = false,
        Err(e) => view.problem = Some(err(e)),
    }
    view
}

#[tauri::command]
pub fn font_status(path: String) -> Reply<FontView> {
    Ok(font_view(&open(&path)?))
}

/// The images in the game that could be its font, best first, each with the grids that fit it.
#[tauri::command]
pub fn font_candidates(path: String) -> Reply<Vec<SheetCandidateView>> {
    let project = open(&path)?;
    let archive = project.original().map_err(err)?;
    let candidates = project.font_candidates().map_err(err)?;

    Ok(candidates
        .into_iter()
        .map(|c| SheetCandidateView {
            image: archive
                .get(&c.entry)
                .map(|e| data_uri(&e.data))
                .unwrap_or_default(),
            grids: c
                .grids
                .iter()
                .map(|g| GridSuggestionView {
                    grid: GridView::from(g.grid),
                    fit: g.fit,
                    capacity: g.grid.capacity(),
                })
                .collect(),
            entry: c.entry,
            width: c.width,
            height: c.height,
            ink_share: c.ink_share,
            colours: c.colours,
        })
        .collect())
}

/// Says which entry holds the font and how it is laid out.
///
/// The grid is given, not guessed: the interface offers the ranked suggestions and a person picks
/// one, because a grid off by a pixel shifts every glyph and reads as a rendering bug rather than
/// as a wrong setting.
#[tauri::command]
pub fn set_font_sheet(
    path: String,
    entry: String,
    grid: GridView,
    order: Option<String>,
) -> Reply<FontView> {
    let mut project = open(&path)?;
    let previous = project.profile().font.clone();
    project.profile_mut().font = Some(FontProfile {
        entry,
        grid: Some(grid.into()),
        order: order.unwrap_or_default(),
        device_font: false,
        // A folder of fonts is the person's, not the sheet's; changing which image is the font
        // does not un-choose it.
        mark_library: previous.as_ref().and_then(|p| p.mark_library.clone()),
        marks_from: previous.and_then(|p| p.marks_from),
    });
    project.save().map_err(err)?;
    Ok(font_view(&project))
}

/// Records that the game draws with the handset's own font.
///
/// Then there is no sheet to extend and nothing for this tab to compose - which is worth saying
/// plainly, because it is the one case where the answer is "your font is already fine".
#[tauri::command]
pub fn set_device_font(path: String) -> Reply<FontView> {
    let mut project = open(&path)?;
    project.profile_mut().font = Some(FontProfile {
        entry: String::new(),
        grid: None,
        order: String::new(),
        device_font: true,
        mark_library: None,
        marks_from: None,
    });
    project.save().map_err(err)?;
    Ok(font_view(&project))
}

/// Forgets what the project was told about the font.
#[tauri::command]
pub fn clear_font(path: String) -> Reply<FontView> {
    let mut project = open(&path)?;
    project.profile_mut().font = None;
    project.save().map_err(err)?;
    Ok(font_view(&project))
}

/// Measures the fonts in a folder against this game's sheet, best first.
///
/// Measured rather than read off the file: at twelve pixels a well-drawn typeface may contribute
/// a third of its marks and a plainer one two thirds, and nothing about the file says which. That
/// costs a rasterisation of 134 letters per font, so a folder is sampled up to `limit` and the
/// interface says how many of the covering fonts were actually tried.
#[tauri::command]
pub fn scan_font_library(path: String, directory: String, limit: Option<usize>) -> Reply<FontScan> {
    let mut project = open(&path)?;
    let sheet = project
        .font_sheet()
        .map_err(err)?
        .ok_or("say which image holds the font first")?;

    let found = library::scan(Path::new(&directory)).map_err(err)?;
    let covering: Vec<library::Candidate> = found
        .iter()
        .filter(|c| c.covers_vietnamese)
        .cloned()
        .collect();
    let limit = limit.unwrap_or(40).max(1);
    let measured = covering.iter().take(limit).cloned().collect::<Vec<_>>();
    let fits = library::rank(&sheet, &measured).map_err(err)?;

    // Remembering the folder is the point of choosing one; the font itself is chosen separately,
    // because a person may want to look at several before deciding.
    let chosen = project
        .profile()
        .font
        .as_ref()
        .and_then(|f| f.marks_from.clone());
    if let Some(profile) = project.profile_mut().font.as_mut() {
        profile.mark_library = Some(PathBuf::from(&directory));
    }
    project.save().map_err(err)?;

    Ok(FontScan {
        found: found.len(),
        covering: covering.len(),
        measured: measured.len(),
        fonts: fits
            .into_iter()
            .map(|f| FontFitView {
                chosen: chosen.as_deref() == Some(f.path.as_path()),
                path: f.path.display().to_string(),
                name: f.name.clone(),
                share: f.share(),
                from_typeface: f.from_typeface,
                composed: f.composed,
            })
            .collect(),
    })
}

/// Chooses the typeface the diacritics are borrowed from, or goes back to the drawn ones.
///
/// Nothing is copied. The path is remembered and the font is read from where its owner keeps it,
/// so a project can be sent to a translator without carrying somebody's typefaces along.
#[tauri::command]
pub fn set_marks_font(path: String, font: Option<String>) -> Reply<FontView> {
    let mut project = open(&path)?;
    let font = font.filter(|f| !f.trim().is_empty()).map(PathBuf::from);
    if let Some(chosen) = &font {
        // Read it now rather than at compose time: a font that cannot be read should be refused
        // where the person chose it, not three screens later.
        MarkSource::from_path(chosen).map_err(err)?;
    }
    project
        .profile_mut()
        .font
        .as_mut()
        .ok_or("say which image holds the font first")?
        .marks_from = font;
    project.save().map_err(err)?;
    Ok(font_view(&project))
}

/// Builds the extended sheet and writes it into the project's `fonts/` directory.
///
/// It does not install it. Making the game *use* the new glyphs means changing how it looks them
/// up, which is per-game, and saying otherwise would be the difference between a font that works
/// and one that looks like it should.
#[tauri::command]
pub fn compose_font(path: String) -> Reply<CompositionView> {
    let project = open(&path)?;
    let marks = project
        .profile()
        .font
        .as_ref()
        .and_then(|f| f.marks_from.clone())
        .map(|p| MarkSource::from_path(&p))
        .transpose()
        .map_err(err)?;

    let (written, report) = project
        .compose_font(marks.as_ref())
        .map_err(err)?
        .ok_or("say which image holds the font first")?;

    Ok(CompositionView {
        image: std::fs::read(&written)
            .map(|b| data_uri(&b))
            .unwrap_or_default(),
        path: written.display().to_string(),
        added: report.added.iter().collect(),
        skipped: {
            // Kept in the order the reasons first appeared rather than sorted: the first one is
            // the one that stopped the most letters, and that is the one worth reading.
            let mut groups: Vec<SkippedGroupView> = Vec::new();
            for skip in &report.skipped {
                match groups.iter_mut().find(|g| g.reason == skip.reason) {
                    Some(group) => group.letters.push(skip.composed),
                    None => groups.push(SkippedGroupView {
                        reason: skip.reason.clone(),
                        letters: skip.composed.to_string(),
                    }),
                }
            }
            groups
        },
        from_typeface: report.from_typeface,
        typeface: report.typeface,
    })
}

/// Renders sample text with the drawn marks and, when one is chosen, with the typeface's.
///
/// Which reads better is not a thing a count can answer, so it is put in front of a person at the
/// size that ships.
#[tauri::command]
pub fn font_preview(path: String, text: Option<String>, scale: Option<u32>) -> Reply<String> {
    let project = open(&path)?;
    let text = text.unwrap_or_else(|| {
        "Cá đã cắn câu\nBạn nhận được 5 vàng\nĐiểm kinh nghiệm\nThoát trò chơi".to_string()
    });
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let written = project
        .preview_font(&lines, scale.unwrap_or(4).clamp(1, 8))
        .map_err(err)?
        .ok_or("say which image holds the font first")?;
    std::fs::read(&written).map(|b| data_uri(&b)).map_err(err)
}

#[cfg(test)]
mod tests {
    /// The base64 here is written out by hand, and hand-written base64 gets the tail wrong: one
    /// or two leftover bytes need a different number of characters and padding signs. A wrong
    /// tail shows as a picture that will not load, with nothing on screen to say why.
    #[test]
    fn images_encode_the_way_a_browser_expects() {
        let cases: [(&[u8], &str); 5] = [
            (b"", ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                super::data_uri(input),
                format!("data:image/png;base64,{expected}"),
                "encoding {input:?}"
            );
        }
    }

    /// Every byte value, round-tripped through a decoder written independently of the encoder.
    #[test]
    fn every_byte_survives_the_round_trip() {
        let bytes: Vec<u8> = (0..=255u8).collect();
        for length in [1, 2, 3, 254, 255, 256] {
            let source = &bytes[..length];
            let encoded = super::data_uri(source);
            let encoded = encoded.strip_prefix("data:image/png;base64,").unwrap();

            let mut bits = 0u32;
            let mut held = 0u32;
            let mut decoded = Vec::new();
            for c in encoded.chars().filter(|c| *c != '=') {
                let value = match c {
                    'A'..='Z' => c as u32 - 'A' as u32,
                    'a'..='z' => c as u32 - 'a' as u32 + 26,
                    '0'..='9' => c as u32 - '0' as u32 + 52,
                    '+' => 62,
                    '/' => 63,
                    other => panic!("{other:?} is not a base64 character"),
                };
                bits = bits << 6 | value;
                held += 6;
                if held >= 8 {
                    held -= 8;
                    decoded.push((bits >> held & 0xFF) as u8);
                }
            }
            assert_eq!(decoded, source, "at length {length}");
        }
    }
}
