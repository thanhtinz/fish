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
use tjlocalizer_core::claude::{self, Analyst};
use tjlocalizer_core::font::library;
use tjlocalizer_core::font::outline::MarkSource;
use tjlocalizer_core::lang::{known_languages, Language};
use tjlocalizer_core::project::{FontProfile, Project};
use tjlocalizer_core::provider::{Briefing, HttpProvider, ProviderConfig, ProviderKind};
use tjlocalizer_core::register;
use tjlocalizer_core::secrets::Keys;
use tjlocalizer_core::translate::Provider as _;
use tjlocalizer_core::translate::{self, DictionaryProvider, Request};
use tjlocalizer_core::tree;
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

/// Imports a game that is installed on disk as a directory.
///
/// The tree is walked without opening anything, and only the files in a format this build reads
/// are copied in. What comes back says how many files the game holds and how few of them were
/// read, because "23 files" on its own reads like a mistake and "41 812 files, 23 read" does not.
#[tauri::command]
pub fn import_tree(
    app: tauri::AppHandle,
    game_path: String,
    into: String,
    name: Option<String>,
    targets: Vec<String>,
) -> Reply<IngestView> {
    let game = Path::new(&game_path);
    let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
        game.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "game".to_string())
    });
    let root = Path::new(&into).join(&name);

    let (mut project, ingested) =
        Project::create_from_tree(&root, &name, game, &tree::Limits::default()).map_err(err)?;

    if !targets.is_empty() {
        project.profile_mut().targets = targets
            .iter()
            .map(|tag| {
                let language = Language::new(tag.as_str());
                let style = default_style(&language);
                tjlocalizer_core::project::Target::new(language, style)
            })
            .collect();
        project.save().map_err(err)?;
    }

    let dir = config_dir(&app);
    let summary = ProjectSummary::of(&project);
    Recents::load(&dir).remember(&dir, &summary.path);

    Ok(IngestView {
        project: summary,
        scanned: ingested.scanned,
        total_size: ingested.total_size,
        read: ingested.files.len(),
        read_size: ingested.files.iter().map(|f| f.size).sum(),
        evidence: ingested.evidence,
        skipped: ingested
            .skipped
            .into_iter()
            .map(|s| SkippedView {
                path: s.path,
                size: s.size,
                reason: s.reason,
            })
            .collect(),
    })
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
    // Measured once for the whole table rather than per row: reading the widths means bounding
    // the ink in every cell of the sheet, and a table has thousands of rows.
    let metrics = project.font_metrics().map_err(err)?;

    Ok(graph
        .nodes
        .iter()
        .map(|node| {
            NodeView::measured(
                node,
                approved.get(&node.id),
                by_node.get(node.id.as_str()).copied(),
                &from,
                &language,
                metrics.as_ref(),
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

    let voice = project.voice(&node_id).map_err(err)?;
    let request = Request {
        source_text: node.source_text.clone(),
        from: project.source_language().clone(),
        to: language,
        context: format!("{:?}", node.context).to_lowercase(),
        placeholders: node.constraints.placeholders.clone(),
        // Who is speaking decides the pronouns, and in Vietnamese there is no neutral choice to
        // fall back on: a line a character says, translated as interface text, addresses the
        // player as nobody. What the inference worked out is used where it worked something out,
        // and the game's own voice where it did not.
        speaker: voice.0,
        stance: voice.1,
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
    let voice = project.voice(&node_id).map_err(err)?;
    let request = Request {
        source_text: node.source_text.clone(),
        from: project.source_language().clone(),
        to: language,
        context: format!("{:?}", node.context).to_lowercase(),
        placeholders: node.constraints.placeholders.clone(),
        // Who is speaking decides the pronouns, and in Vietnamese there is no neutral choice to
        // fall back on: a line a character says, translated as interface text, addresses the
        // player as nobody. What the inference worked out is used where it worked something out,
        // and the game's own voice where it did not.
        speaker: voice.0,
        stance: voice.1,
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

/// Shorter ways of saying what this row says, narrowest first (§24).
///
/// Offered, never applied. Each one names where it came from - another reading in the project's
/// dictionary, a pronoun its interface register says to drop - because a suggestion a translator
/// cannot check is one they have to work out again from scratch.
#[tauri::command]
pub fn shorter(path: String, language: String, node_id: String) -> Reply<Vec<AlternativeView>> {
    let project = open(&path)?;
    Ok(project
        .shorter_alternatives(&Language::new(language), &node_id)
        .map_err(err)?
        .into_iter()
        .map(AlternativeView::from)
        .collect())
}

/// Draws one string exactly as the game will draw it.
///
/// The numbers beside a row say a translation is wider; this says what it looks like. Which
/// matters most for Vietnamese, where the risk is not only width: a mark can land on the letter
/// below it, or a stack can read as a smudge at twelve pixels, and no count sees either.
#[tauri::command]
pub fn render_text(path: String, text: String, scale: Option<u32>) -> Reply<Option<String>> {
    let project = open(&path)?;
    let Some(sheet) = project.font_sheet_for_preview().map_err(err)? else {
        return Ok(None);
    };
    if text.is_empty() {
        return Ok(None);
    }
    let metrics = tjlocalizer_core::font::metrics::Metrics::of(&sheet);
    let line = tjlocalizer_core::font::sheet::render_line_with(&sheet, &metrics, &text);
    let image = tjlocalizer_core::font::sheet::scaled(&line, scale.unwrap_or(3).clamp(1, 8));
    Ok(Some(data_uri(&image.encode_png().map_err(err)?)))
}

/// Draws every approved translation for one language, original above translation.
#[tauri::command]
pub fn proof_sheet(path: String, language: String, scale: Option<u32>) -> Reply<Option<String>> {
    let project = open(&path)?;
    let language = Language::new(language);
    let Some(written) = project
        .proof_sheet(&language, scale.unwrap_or(4))
        .map_err(err)?
    else {
        return Ok(None);
    };
    std::fs::read(&written)
        .map(|b| Some(data_uri(&b)))
        .map_err(err)
}

/// How this game draws its text, and what could be handed to the handset's own font (§16).
///
/// The other route to Vietnamese, and for a game whose sheet is CJK-only the only one: instead of
/// composing 134 letters into the sheet and teaching the game it grew, stop using the sheet.
#[tauri::command]
pub fn system_font(path: String) -> Reply<SystemFontView> {
    let project = open(&path)?;
    let strategy = project.font_strategy().map_err(err)?;
    Ok(SystemFontView {
        bitmap: strategy.bitmap,
        device: strategy.device,
        evidence: strategy.evidence,
        switched: project.switched_to_device_font().map_err(err)?,
        candidates: project
            .system_font_candidates()
            .map_err(err)?
            .into_iter()
            .map(|found| SystemFontCandidateView {
                class: found.class,
                method: found.method,
                descriptor: found.descriptor,
                job: found.job.key().to_string(),
                evidence: found.evidence,
            })
            .collect(),
    })
}

/// Writes the rules that make that switch, all switched off (§16, §19).
#[tauri::command]
pub fn write_system_font_rules(path: String) -> Reply<Vec<RuleView>> {
    let project = open(&path)?;
    project.write_system_font_rules().map_err(err)?;
    rules(path)
}

/// Where the game looks like it writes down the shape of its own glyph sheet (§16).
///
/// The half of a font swap that is per-game. It still cannot be known from here - it can be
/// looked for, and a class holding the sheet's row count or a string listing its characters in
/// order is almost always it. Evidence for a person to read, never a patch.
#[tauri::command]
pub fn font_lookup_candidates(path: String) -> Reply<Vec<FontLookupView>> {
    let project = open(&path)?;
    Ok(project
        .font_lookup_candidates()
        .map_err(err)?
        .into_iter()
        .map(|candidate| FontLookupView {
            class: candidate.class,
            what: candidate.what.key().to_string(),
            value: candidate.value,
        })
        .collect())
}

/// Compares the drawing against the one somebody accepted, and marks what moved (§25).
///
/// The failure this catches is not a wrong translation - it is everything else moving. Six lines
/// were edited and sixty changed: a font was recomposed, a glyph order edited, a rule installed a
/// sheet whose letters sit a pixel lower. No text report shows that.
#[tauri::command]
pub fn visual_regression(
    path: String,
    language: String,
    scale: Option<u32>,
) -> Reply<RegressionView> {
    let project = open(&path)?;
    let language = Language::new(language);
    let scale = scale.unwrap_or(4);

    let Some((difference, picture)) = project.visual_regression(&language, scale).map_err(err)?
    else {
        return Ok(RegressionView {
            compared: false,
            identical: false,
            resized: false,
            changed: 0,
            share: 0.0,
            bands: Vec::new(),
            picture: None,
        });
    };

    Ok(RegressionView {
        compared: true,
        identical: difference.is_identical(),
        resized: difference.resized,
        changed: difference.changed,
        share: difference.share(),
        bands: difference
            .bands
            .iter()
            .map(|b| format!("{}-{}", b.top, b.bottom))
            .collect(),
        picture: std::fs::read(&picture).ok().map(|b| data_uri(&b)),
    })
}

/// Accepts the current drawing as what this language should look like from now on.
#[tauri::command]
pub fn accept_baseline(path: String, language: String, scale: Option<u32>) -> Reply<bool> {
    let project = open(&path)?;
    let language = Language::new(language);
    Ok(project
        .accept_baseline(&language, scale.unwrap_or(4))
        .map_err(err)?
        .is_some())
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

// ---------------------------------------------------------------------------------------------
// Images with words painted into them (§17).
//
// A game's buttons are often artwork with the words already drawn on, and no amount of
// translating strings touches them - so the images are shown, a person decides, and the decision
// is recorded where the build can hold the project to it.
//
// Where the project knows the game's glyph sheet, the words can be read out of the picture by
// matching it against those same letters. What matches is offered; what does not is shown as
// unread rather than guessed, and nothing is written into the project without somebody accepting
// it.
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub fn image_assets(path: String) -> Reply<Vec<ImageAssetView>> {
    let project = open(&path)?;
    let archive = project.original().map_err(err)?;
    let marked = project.profile().text_assets.clone();

    Ok(project
        .image_assets()
        .map_err(err)?
        .into_iter()
        .map(|asset| {
            let known = marked.iter().find(|t| t.entry == asset.entry);
            ImageAssetView {
                image: archive
                    .get(&asset.entry)
                    .map(|e| data_uri(&e.data))
                    .unwrap_or_default(),
                says: known.map(|t| t.says.clone()),
                replacement: known.and_then(|t| t.replacement.clone()),
                marked: known.is_some(),
                entry: asset.entry,
                width: asset.width,
                height: asset.height,
                colours: asset.colours,
                hints: asset.hints,
            }
        })
        .collect())
}

/// Reads the words out of images with the game's own letters (§17).
///
/// An empty list means every image whose shape suggests a label; naming entries reads those
/// whatever their shape.
#[tauri::command]
pub fn read_text_assets(path: String, entries: Vec<String>) -> Reply<Vec<ReadingView>> {
    let project = open(&path)?;
    let Some(readings) = project.read_text_assets(&entries).map_err(err)? else {
        return Err(
            "Chưa biết ảnh nào là font của game, nên không có chữ nào để đối chiếu. \
                    Chọn font ở tab Font trước."
                .into(),
        );
    };
    Ok(readings
        .into_iter()
        .map(|reading| ReadingView {
            complete: reading.is_complete(),
            text: reading.text(),
            confidence: reading.confidence,
            unread: reading.unread,
            entry: reading.entry,
        })
        .collect())
}

#[tauri::command]
pub fn mark_text_asset(
    path: String,
    entry: String,
    says: Option<String>,
    replacement: Option<String>,
) -> Reply<Vec<ImageAssetView>> {
    let mut project = open(&path)?;
    project
        .mark_text_asset(tjlocalizer_core::assets::TextAsset {
            entry,
            says: says.unwrap_or_default(),
            replacement: replacement.filter(|r| !r.trim().is_empty()),
        })
        .map_err(err)?;
    image_assets(path)
}

#[tauri::command]
pub fn unmark_text_asset(path: String, entry: String) -> Reply<Vec<ImageAssetView>> {
    let mut project = open(&path)?;
    if !project.unmark_text_asset(&entry).map_err(err)? {
        return Err(format!("{entry} was not marked"));
    }
    image_assets(path)
}

// ---------------------------------------------------------------------------------------------
// What a line is for and who says it (§10, §5, §15).
//
// Read from the lines around it, because one string on its own often settles nothing: `Yes` is a
// button by its length and a reply by its company. What comes back is readings with their
// evidence and a cast with theirs - nothing here decides a register, which §14 leaves to a person.
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub fn context(path: String) -> Reply<ContextView> {
    let project = open(&path)?;
    let inference = project.inference().map_err(err)?;
    Ok(ContextView {
        readings: inference.readings.len(),
        cast: inference
            .cast
            .into_iter()
            .map(|character| CharacterView {
                name: character.name,
                lines: character.lines,
                appears_in: character.appears_in,
                beside: character.beside,
                stance: character
                    .suggested_stance
                    .as_ref()
                    .map(|hint| match hint.stance {
                        tjlocalizer_core::register::Stance::Deferential => "kính cẩn".to_string(),
                        tjlocalizer_core::register::Stance::Familiar => "thân mật".to_string(),
                        tjlocalizer_core::register::Stance::Hostile => "thù địch".to_string(),
                        tjlocalizer_core::register::Stance::Neutral => "trung tính".to_string(),
                    }),
                because: character
                    .suggested_stance
                    .map(|hint| hint.because)
                    .unwrap_or_default(),
            })
            .collect(),
    })
}

// ---------------------------------------------------------------------------------------------
// Adapters written as data (§20).
//
// A plugin says what to look for in one game or one engine and what to conclude. It is data and
// only data - nothing in it is executed - so what it can contribute is exactly what this build
// can already do to any archive, and the panel shows both what it claims and whether any of it
// applied here.
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub fn plugins(path: String) -> Reply<PluginsView> {
    let project = open(&path)?;
    let plugins = project.plugins().map_err(err)?;
    let archive = project.original().map_err(err)?;
    let fired = plugins.capabilities(&archive);

    let count = |pattern: &str| {
        archive
            .entries()
            .iter()
            .filter(|e| tjlocalizer_core::plugin::glob(pattern, &e.name))
            .count()
    };

    Ok(PluginsView {
        loaded: plugins
            .loaded
            .iter()
            .map(|plugin| PluginView {
                id: plugin.id.clone(),
                description: plugin.description.clone(),
                author: plugin.author.clone(),
                path: plugin.path.display().to_string(),
                capabilities: plugin
                    .capabilities
                    .iter()
                    .map(|rule| PluginClaimView {
                        what: rule.id.clone(),
                        detail: format!("{:.0}%", rule.confidence * 100.0),
                        matches: usize::from(fired.iter().any(|c| c.id == rule.id)),
                    })
                    .collect(),
                resources: plugin
                    .resources
                    .iter()
                    .map(|resource| PluginClaimView {
                        what: resource.pattern.clone(),
                        detail: resource.format.clone(),
                        matches: count(&resource.pattern),
                    })
                    .collect(),
                fonts: plugin
                    .fonts
                    .iter()
                    .map(|font| PluginClaimView {
                        what: font.pattern.clone(),
                        detail: format!(
                            "{}x{}, {} cột",
                            font.cell_width, font.cell_height, font.columns
                        ),
                        matches: count(&font.pattern),
                    })
                    .collect(),
                rules: plugin
                    .rules
                    .iter()
                    .map(|rule| format!("{}:{}", plugin.id, rule.id))
                    .collect(),
                dictionary_entries: plugin
                    .dictionary
                    .as_ref()
                    .map(|pack| pack.entries.len())
                    .unwrap_or(0),
                problems: plugin.problems(),
            })
            .collect(),
        broken: plugins
            .broken
            .iter()
            .map(|(path, reason)| BrokenPluginView {
                path: path.display().to_string(),
                reason: reason.clone(),
            })
            .collect(),
    })
}

// ---------------------------------------------------------------------------------------------
// Per-game patches (§19).
//
// Everything else the application does works on any JAR. These do not: they change one game,
// because they were written for one game. So they are listed with what they would do and why
// they might not, and nothing runs until somebody switches it on.
// ---------------------------------------------------------------------------------------------

#[tauri::command]
pub fn rules(path: String) -> Reply<Vec<RuleView>> {
    let project = open(&path)?;
    Ok(project
        .plan_rules()
        .map_err(err)?
        .into_iter()
        .map(RuleView::from)
        .collect())
}

/// Writes the rule that puts the composed sheet into the game.
///
/// Switched off, as every generated rule is. It also only covers the half of the job that is the
/// same in every game - replacing the image - and its description says which half is missing,
/// because a rule that swapped the artwork and stopped would leave the game drawing its old
/// letters from a taller sheet.
#[tauri::command]
pub fn write_font_install_rule(path: String) -> Reply<Vec<RuleView>> {
    let project = open(&path)?;
    let rule = project.font_install_rule().map_err(err)?;
    project.put_rule(rule).map_err(err)?;
    rules(path)
}

#[tauri::command]
pub fn set_rule_enabled(path: String, id: String, enabled: bool) -> Reply<Vec<RuleView>> {
    let project = open(&path)?;
    if !project.set_rule_enabled(&id, enabled).map_err(err)? {
        return Err(format!("this project has no rule {id}"));
    }
    rules(path)
}

#[tauri::command]
pub fn remove_rule(path: String, id: String) -> Reply<Vec<RuleView>> {
    let project = open(&path)?;
    if !project.remove_rule(&id).map_err(err)? {
        return Err(format!("this project has no rule {id}"));
    }
    rules(path)
}

/// The models offered for analysis.
///
/// Named rather than free text: a mistyped model is a failed call whose message says nothing
/// useful, and the choice that matters - a cheaper one for a large scan - is one of two.
const MODELS: &[&str] = &["claude-opus-5", "claude-haiku-4-5"];

#[tauri::command]
pub fn analyst(app: tauri::AppHandle, path: String) -> Reply<AnalystView> {
    let project = open(&path)?;
    let settings = project.profile().claude.clone().unwrap_or_default();
    Ok(AnalystView {
        enabled: settings.enabled,
        endpoint: claude::ENDPOINT.to_string(),
        model: settings.model,
        has_key: Keys::load(&config_dir(&app)).has(claude::ENDPOINT),
        models: MODELS.iter().map(|m| m.to_string()).collect(),
    })
}

#[tauri::command]
pub fn set_analyst(
    app: tauri::AppHandle,
    path: String,
    model: String,
    enabled: bool,
) -> Reply<AnalystView> {
    let mut project = open(&path)?;
    let mut settings = project.profile().claude.clone().unwrap_or_default();
    settings.model = if model.trim().is_empty() {
        claude::DEFAULT_MODEL.to_string()
    } else {
        model
    };
    settings.enabled = enabled;
    project.profile_mut().claude = Some(settings);
    project.save().map_err(err)?;
    analyst(app, path)
}

/// Stores the key for the analysis endpoint.
///
/// The same endpoint the `anthropic` translation engine uses, and `secrets::Keys` is keyed by
/// endpoint - so a key entered on either screen is found by both.
#[tauri::command]
pub fn set_analyst_key(app: tauri::AppHandle, path: String, key: String) -> Reply<AnalystView> {
    let _ = open(&path)?;
    let dir = config_dir(&app);
    let mut keys = Keys::load(&dir);
    keys.set(claude::ENDPOINT, &key);
    keys.save(&dir).map_err(err)?;
    analyst(app, path)
}

/// What a scan would send, without sending it.
///
/// The names, in full, because they are the thing being consented to. The token count comes from
/// the service's own counting endpoint - which does mean one call, and it carries no file names
/// this preview is not already showing.
#[tauri::command]
pub fn scan_preview(app: tauri::AppHandle, path: String) -> Reply<ScanPreview> {
    let project = open(&path)?;
    let settings = project.profile().claude.clone().unwrap_or_default();
    let archive = project.original().map_err(err)?;
    let facts = claude::facts(&archive);
    let paths: Vec<String> = facts.iter().map(|f| f.path.clone()).collect();

    let mut preview = ScanPreview {
        paths,
        model: settings.model.clone(),
        tokens: None,
        trouble: String::new(),
    };

    let key = Keys::load(&config_dir(&app))
        .get(claude::ENDPOINT)
        .map(|k| k.to_string());
    match (settings.enabled, key) {
        (true, Some(key)) => {
            let analyst = Analyst::new(settings);
            let mut total = 0;
            for batch in facts.chunks(claude::BATCH) {
                let call = analyst.survey_call(&key, batch);
                match analyst.count(&key, &analyst.count_call(&key, &call)) {
                    Ok(count) => total += count,
                    Err(why) => {
                        preview.trouble = why;
                        return Ok(preview);
                    }
                }
            }
            preview.tokens = Some(total);
        }
        (false, _) => preview.trouble = claude::OFF.to_string(),
        (_, None) => preview.trouble = "no key stored for this endpoint".to_string(),
    }
    Ok(preview)
}

/// Runs a scan and files what came back.
///
/// The result is stored apart from the package survey and returned apart from it, because a guess
/// filed beside a finding becomes indistinguishable from one within a week.
#[tauri::command]
pub fn scan(app: tauri::AppHandle, path: String) -> Reply<Vec<SuggestionView>> {
    let project = open(&path)?;
    let settings = project.profile().claude.clone().unwrap_or_default();
    if !settings.enabled {
        return Err(claude::OFF.to_string());
    }
    let key = Keys::load(&config_dir(&app))
        .get(claude::ENDPOINT)
        .map(|k| k.to_string())
        .ok_or("no key stored for this endpoint")?;

    let archive = project.original().map_err(err)?;
    let facts = claude::facts(&archive);
    let analyst = Analyst::new(settings);
    let (survey, trouble) = analyst.survey_all(&key, &facts);
    // Nothing came back at all: say why rather than showing an empty list, which reads as "there
    // is nothing here".
    if survey.verdicts.is_empty() && !trouble.is_empty() {
        return Err(trouble.join("; "));
    }
    project.save_suggestions(&survey).map_err(err)?;
    Ok(suggestions_of(&survey))
}

/// The last scan's suggestions, without running one.
#[tauri::command]
pub fn suggestions(path: String) -> Reply<Vec<SuggestionView>> {
    let project = open(&path)?;
    Ok(project
        .suggestions()
        .map_err(err)?
        .as_ref()
        .map(suggestions_of)
        .unwrap_or_default())
}

fn suggestions_of(survey: &tjlocalizer_core::claude::Survey) -> Vec<SuggestionView> {
    let mut shown: Vec<SuggestionView> = survey
        .verdicts
        .iter()
        .filter(|v| v.holds_text)
        .map(|v| SuggestionView {
            path: v.path.clone(),
            why: v.why.clone(),
            confidence: v.confidence,
            model: survey.model.clone(),
        })
        .collect();
    shown.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    shown
}

/// Asks what one entry is. The only command that sends any of a game's bytes.
#[tauri::command]
pub fn inspect_entry(
    app: tauri::AppHandle,
    path: String,
    entry: String,
) -> Reply<tjlocalizer_core::claude::Inspection> {
    let project = open(&path)?;
    let settings = project.profile().claude.clone().unwrap_or_default();
    if !settings.enabled {
        return Err(claude::OFF.to_string());
    }
    let key = Keys::load(&config_dir(&app))
        .get(claude::ENDPOINT)
        .map(|k| k.to_string())
        .ok_or("no key stored for this endpoint")?;

    let archive = project.original().map_err(err)?;
    let found = archive
        .get(&entry)
        .ok_or_else(|| format!("no entry named {entry:?} in this package"))?;
    Analyst::new(settings).inspect(&key, &claude::Sample::of(&entry, &found.data))
}

/// Asks what looks wrong with the approved translations of one language.
///
/// Sends the game's own text, so the interface asks first and says how many lines. What comes back
/// are notes on rows; nothing here writes to a translation store.
#[tauri::command]
pub fn review_language(
    app: tauri::AppHandle,
    path: String,
    language: String,
    limit: usize,
) -> Reply<Vec<tjlocalizer_core::claude::ReviewNote>> {
    let project = open(&path)?;
    let settings = project.profile().claude.clone().unwrap_or_default();
    if !settings.enabled {
        return Err(claude::OFF.to_string());
    }
    let key = Keys::load(&config_dir(&app))
        .get(claude::ENDPOINT)
        .map(|k| k.to_string())
        .ok_or("no key stored for this endpoint")?;

    let language = Language::new(&language);
    let graph = project.graph().map_err(err)?;
    let store = project.translations(&language).map_err(err)?;
    let glossary = project.glossary(&language).map_err(err)?;
    let style = project.style(&language);

    let mut lines = Vec::new();
    for node in graph.translatable() {
        // The store holds only what a person approved, so being in it is the filter.
        let Some(target) = store.get(&node.id) else {
            continue;
        };
        lines.push(tjlocalizer_core::claude::ReviewLine {
            node_id: node.id.clone(),
            context: node.context.key().to_string(),
            source: node.source_text.clone(),
            target: target.to_string(),
        });
        if lines.len() >= limit.max(1) {
            break;
        }
    }
    if lines.is_empty() {
        return Ok(Vec::new());
    }

    // Only the terms that occur in what is being sent; the whole glossary would be a larger
    // request saying no more.
    let mut terms: Vec<(String, String)> = Vec::new();
    for line in &lines {
        for term in glossary.matches_in(&line.source) {
            let pair = (term.source.clone(), term.target.clone());
            if !terms.contains(&pair) {
                terms.push(pair);
            }
        }
    }

    Analyst::new(settings).review(
        &key,
        &lines,
        style.as_ref().map(|s| s.description.as_str()),
        &terms,
    )
}

/// What applying the current patch would overwrite. Writes nothing.
#[tauri::command]
pub fn plan_patch(path: String, language: String, game: String) -> Reply<PatchPlanView> {
    let project = open(&path)?;
    let plan = project
        .plan_patch(&Language::new(&language), Path::new(&game))
        .map_err(err)?;
    Ok(PatchPlanView {
        applicable: plan.is_applicable(),
        ready: plan.ready.into_iter().map(|c| c.path).collect(),
        mismatched: plan
            .mismatched
            .into_iter()
            .map(|m| MismatchView {
                path: m.path,
                reason: m.reason,
            })
            .collect(),
    })
}

/// Writes the patch into a game directory.
///
/// Separate from `plan_patch` on purpose: the interface shows what would be overwritten and asks,
/// and this only runs after somebody said yes. What it replaces is kept under `builds/`.
#[tauri::command]
pub fn apply_patch(path: String, language: String, game: String) -> Reply<Vec<String>> {
    let project = open(&path)?;
    project
        .apply_patch(&Language::new(&language), Path::new(&game))
        .map_err(err)
}

/// What has been done to this project, most recent last.
#[tauri::command]
pub fn journal(path: String, limit: usize) -> Reply<Vec<JournalView>> {
    let project = open(&path)?;
    Ok(
        tjlocalizer_core::journal::tail(project.root(), limit.max(1))
            .into_iter()
            .map(|e| JournalView {
                at: e.at,
                kind: e.kind,
                language: e.language,
                detail: e.detail,
            })
            .collect(),
    )
}

/// Adds the line no recorded milestone can know: why you stopped.
#[tauri::command]
pub fn add_note(path: String, text: String) -> Reply<Vec<JournalView>> {
    let project = open(&path)?;
    if text.trim().is_empty() {
        return Err("a note with nothing in it is not worth keeping".into());
    }
    project.note(text.trim()).map_err(err)?;
    journal(path, 12)
}

/// Looks for a J2ME emulator already on this machine. Downloads nothing.
#[tauri::command]
pub fn find_emulators(path: String) -> Reply<EmulatorSearch> {
    let project = open(&path)?;
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);

    Ok(EmulatorSearch {
        found: tjlocalizer_core::emulator::find(home.as_deref())
            .into_iter()
            .map(|f| EmulatorView {
                name: f.name.to_string(),
                path: f.path.display().to_string(),
                evidence: f.evidence,
            })
            .collect(),
        // Carried whether or not anything was found: somebody whose emulator was missed needs to
        // see that the search never looked where it is.
        searched: tjlocalizer_core::emulator::searched(home.as_deref())
            .into_iter()
            .map(|p| p.display().to_string())
            .collect(),
        java_available: tjlocalizer_core::emulator::java_available(),
        configured: project
            .profile()
            .emulator
            .as_ref()
            .map(|e| e.command.clone()),
    })
}

/// Records one of the found emulators for this project.
#[tauri::command]
pub fn use_emulator(path: String, emulator_path: String) -> Reply<EmulatorSearch> {
    let mut project = open(&path)?;
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);

    let chosen = tjlocalizer_core::emulator::find(home.as_deref())
        .into_iter()
        .find(|f| f.path.display().to_string() == emulator_path)
        .ok_or("that emulator is no longer where it was found")?;

    project.profile_mut().emulator = Some(chosen.emulator);
    project.save().map_err(err)?;
    find_emulators(path)
}

/// Runs the recorded emulator on the newest build.
///
/// Blocks until it exits, which is what a person pressing "play" means: they are going to look at
/// the game and come back.
#[tauri::command]
pub fn play(path: String, language: String) -> Reply<String> {
    let project = open(&path)?;
    let status = project.play(&Language::new(&language)).map_err(err)?;
    if status.success() {
        Ok("the emulator closed normally".into())
    } else {
        // Not an error of this tool's making, and said as such: an emulator exits non-zero for
        // its own reasons, and reporting it as a failure of the build would be wrong.
        Ok(format!("the emulator exited with {status}"))
    }
}
