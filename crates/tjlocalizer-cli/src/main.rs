//! Command line front end (specification §27).
//!
//! Each subcommand is one step of the workflow in §22 and leaves its result on disk, so the steps
//! can be run separately, by different people, days apart. `localize` runs the whole chain.
//!
//! A project can ship in several languages. Commands that act on one take `--lang`; without it
//! they act on the project's first target, and `--all` runs every enabled one.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use tjlocalizer_core::build::Branding;
use tjlocalizer_core::dictionary::Dictionary;
use tjlocalizer_core::graph::ContentGraph;
use tjlocalizer_core::jar::Archive;
use tjlocalizer_core::lang::Language;
use tjlocalizer_core::project::{BuildRecord, Project, Target};
use tjlocalizer_core::register;
use tjlocalizer_core::suggest::{self, Origin};
use tjlocalizer_core::translate::{self, Completeness, DictionaryProvider, Request};
use tjlocalizer_core::validate::{inspect, Severity, ValidationReport};

#[derive(Parser)]
#[command(
    name = "tjlocalizer",
    version,
    about = "Thanhtinz JAR Localizer - Java/J2ME game localization",
    long_about = "Localizes Java and J2ME game archives into one or more languages. Nothing here \
                  is specific to any one game: what a game supports is detected, and what is done \
                  about it comes from the project's rules, dictionaries, glossary and memory."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Import a JAR into a new project directory.
    Import {
        jar: PathBuf,
        /// Where to create the project. Defaults to projects/<name>.
        #[arg(long)]
        into: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
        /// Target languages, comma separated. Defaults to vi-VN.
        #[arg(long, value_delimiter = ',')]
        target: Vec<String>,
        /// The language the game is written in. Detected when not given.
        #[arg(long)]
        source_language: Option<String>,
    },

    /// Detect what the game supports, and write the capability manifest.
    Analyze { project: PathBuf },

    /// Extract translatable text into the content graph.
    Extract { project: PathBuf },

    /// List, add or remove target languages.
    Targets {
        project: PathBuf,
        /// Add a language, optionally as `tag:style-profile`.
        #[arg(long)]
        add: Option<String>,
        /// Remove a language.
        #[arg(long)]
        remove: Option<String>,
    },

    /// Propose translations from the project's memory, glossary and dictionaries.
    Translate {
        project: PathBuf,
        #[arg(long)]
        lang: Option<String>,
        /// Every enabled target.
        #[arg(long)]
        all: bool,
        #[arg(long, default_value_t = 0.75)]
        fuzzy_threshold: f32,
        /// Approve the candidates that restate a decision this project already made: exact memory
        /// hits and locked glossary terms. Fuzzy matches and dictionary glosses never are.
        #[arg(long)]
        apply_safe: bool,
        /// Also show what the offline dictionary engine would propose for untranslated strings.
        #[arg(long)]
        gloss: bool,
    },

    /// Apply approved translations and repackage.
    Build {
        project: PathBuf,
        #[arg(long)]
        lang: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        no_branding: bool,
    },

    /// Copy a built artifact to a path of your choosing.
    Export {
        project: PathBuf,
        /// Where to write it. A directory keeps the built name; anything else is the file name.
        to: PathBuf,
        #[arg(long)]
        lang: Option<String>,
        /// Every enabled target. `to` must then be a directory.
        #[arg(long)]
        all: bool,
        /// Overwrite an existing file.
        #[arg(long)]
        force: bool,
    },

    /// Check a project's latest build, or a JAR on its own.
    Validate {
        target: PathBuf,
        #[arg(long)]
        lang: Option<String>,
    },

    /// Report whether a built archive is ready to launch.
    Test { jar: PathBuf },

    /// List a project's recorded builds.
    Builds {
        project: PathBuf,
        #[arg(long)]
        lang: Option<String>,
    },

    /// Restore an earlier build as the current output.
    Rollback {
        project: PathBuf,
        revision: u32,
        #[arg(long)]
        lang: Option<String>,
    },

    /// Show the dictionaries available, and what they cover.
    Dictionaries {
        /// A project, to include its own packs. Omit for the built-in ones only.
        project: Option<PathBuf>,
    },

    /// List the register profiles this build ships.
    Styles {
        /// Only those for a language.
        #[arg(long)]
        lang: Option<String>,
    },

    /// Import, analyze, extract, propose, build and validate in one pass.
    Localize {
        jar: PathBuf,
        #[arg(long)]
        into: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, value_delimiter = ',', default_value = "vi-VN")]
        target: Vec<String>,
        #[arg(long, default_value = "natural-dialogue")]
        style: String,
        #[arg(long)]
        source_language: Option<String>,
        #[arg(long)]
        no_branding: bool,
    },
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Import {
            jar,
            into,
            name,
            target,
            source_language,
        } => {
            let project = import(&jar, into, name, &target, source_language)?;
            let p = project.profile();
            println!(
                "imported {} into {}",
                jar.display(),
                project.root().display()
            );
            println!("  sha256 {}", p.source.sha256);
            println!(
                "  source {} ({})",
                p.source_language.language.display_name(),
                if p.source_language.detected {
                    "detected"
                } else {
                    "given"
                }
            );
            for t in &p.targets {
                println!("  target {} [{}]", t.language, t.style_profile);
            }
            Ok(())
        }

        Command::Analyze { project } => {
            let project = Project::open(&project)?;
            let manifest = project.analyze()?;
            println!("{} capabilities:", manifest.capabilities.len());
            for capability in &manifest.capabilities {
                println!(
                    "  {:<24} {:.2}  {}",
                    capability.id,
                    capability.confidence,
                    capability.evidence.join(", ")
                );
            }
            Ok(())
        }

        Command::Extract { project } => {
            let project = Project::open(&project)?;
            let graph = project.extract()?;
            report_graph(&graph);
            Ok(())
        }

        Command::Targets {
            project,
            add,
            remove,
        } => {
            let mut project = Project::open(&project)?;
            if let Some(spec) = add {
                let (tag, style) = match spec.split_once(':') {
                    Some((t, s)) => (t.to_string(), s.to_string()),
                    None => (spec.clone(), default_style(&Language::new(&spec))),
                };
                project.add_target(Language::new(tag), &style)?;
            }
            if let Some(tag) = remove {
                project.remove_target(&Language::new(tag))?;
                println!("removed; its translations and builds are left on disk");
            }
            for t in &project.profile().targets {
                println!(
                    "  {:<10} {:<20} {:<18} {}",
                    t.language,
                    t.language.display_name(),
                    t.style_profile,
                    if t.enabled { "enabled" } else { "disabled" }
                );
            }
            Ok(())
        }

        Command::Translate {
            project,
            lang,
            all,
            fuzzy_threshold,
            apply_safe,
            gloss,
        } => {
            let project = Project::open(&project)?;
            for language in languages(&project, lang.as_deref(), all)? {
                println!("== {}", language.display_name());
                let set = project.suggest(&language, fuzzy_threshold)?;
                println!(
                    "{} candidates, {} strings with nothing to propose",
                    set.candidates.len(),
                    set.without_candidate
                );
                for candidate in set.candidates.iter().take(15) {
                    let origin = match candidate.origin {
                        Origin::MemoryExact => "memory".to_string(),
                        Origin::MemoryFuzzy { score } => format!("memory {score:.2}"),
                        Origin::GlossaryTerm => "glossary".to_string(),
                    };
                    println!(
                        "  [{origin}] {:?} -> {:?}",
                        candidate.source, candidate.target
                    );
                }
                if set.candidates.len() > 15 {
                    println!("  ... {} more", set.candidates.len() - 15);
                }

                if apply_safe {
                    let mut approved = project.translations(&language)?;
                    let applied = suggest::apply_safe(&set, &mut approved);
                    project.save_translations(&language, &approved)?;
                    println!("approved {applied} that restate an existing decision");
                }

                if gloss {
                    show_glosses(&project, &language)?;
                }
            }
            Ok(())
        }

        Command::Build {
            project,
            lang,
            all,
            no_branding,
        } => {
            let mut project = Project::open(&project)?;
            if no_branding {
                project.profile_mut().branding = Branding {
                    enabled: false,
                    ..Branding::default()
                };
                project.save()?;
            }
            let mut failed = false;
            for language in languages(&project, lang.as_deref(), all)? {
                let record = project.build(&language)?;
                report_build(&project, &record);
                failed |= !record.validation.is_ok();
            }
            if failed {
                bail!("at least one build did not pass validation");
            }
            Ok(())
        }

        Command::Export {
            project,
            to,
            lang,
            all,
            force,
        } => {
            let project = Project::open(&project)?;
            let langs = languages(&project, lang.as_deref(), all)?;
            if langs.len() > 1 && !to.is_dir() {
                bail!(
                    "exporting {} languages needs {} to be a directory",
                    langs.len(),
                    to.display()
                );
            }
            for language in langs {
                let from = project.output_path(&language)?.with_context(|| {
                    format!("{language} has no build yet - run `tjlocalizer build` first")
                })?;
                let destination = if to.is_dir() {
                    to.join(from.file_name().expect("a built artifact has a name"))
                } else {
                    to.clone()
                };
                if destination.exists() && !force {
                    bail!(
                        "{} already exists; pass --force to overwrite",
                        destination.display()
                    );
                }
                if let Some(parent) = destination.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&from, &destination)?;
                println!("{} -> {}", language, destination.display());
            }
            Ok(())
        }

        Command::Validate { target, lang } => {
            let report = if target.join("project.json").exists() {
                let project = Project::open(&target)?;
                let language = one_language(&project, lang.as_deref())?;
                match project.builds(&language)?.pop() {
                    Some(record) => record.validation,
                    None => bail!("{} has no {language} build to validate", target.display()),
                }
            } else {
                inspect(&read_archive(&target)?)
            };
            report_validation(&report);
            if !report.is_ok() {
                bail!("validation failed");
            }
            Ok(())
        }

        Command::Test { jar } => {
            let archive = read_archive(&jar)?;
            let report = inspect(&archive);
            report_validation(&report);
            println!(
                "{} entries, {} classes",
                archive.entries().len(),
                archive.classes().count()
            );
            println!("note: static checks only - emulator regression is not implemented yet");
            if !report.is_ok() {
                bail!("the archive is not ready to launch");
            }
            Ok(())
        }

        Command::Builds { project, lang } => {
            let project = Project::open(&project)?;
            for language in languages(&project, lang.as_deref(), lang.is_none())? {
                println!("== {language}");
                let builds = project.builds(&language)?;
                if builds.is_empty() {
                    println!("  no builds recorded");
                }
                for record in builds {
                    println!(
                        "  {:04}  {} literals, {} resources  {}  sha256 {}",
                        record.revision,
                        record.report.literals_patched,
                        record.report.resources_patched,
                        if record.validation.is_ok() {
                            "ok  "
                        } else {
                            "FAIL"
                        },
                        record.report.output_sha256
                    );
                }
            }
            Ok(())
        }

        Command::Rollback {
            project,
            revision,
            lang,
        } => {
            let project = Project::open(&project)?;
            let language = one_language(&project, lang.as_deref())?;
            let record = project.rollback(&language, revision)?;
            println!("restored {language} build {:04}", record.revision);
            Ok(())
        }

        Command::Dictionaries { project } => {
            let dictionary: Dictionary = match project {
                Some(path) => Project::open(&path)?.dictionary()?,
                None => tjlocalizer_core::dictionary_data::builtin(),
            };
            println!("{} entries across:", dictionary.entry_count());
            for (from, to) in dictionary.directions() {
                let count = dictionary
                    .packs
                    .iter()
                    .filter(|p| p.from == from && p.to == to)
                    .map(|p| p.entries.len())
                    .sum::<usize>();
                println!(
                    "  {:<10} -> {:<10} {:>5} entries   {} to {}",
                    from.tag(),
                    to.tag(),
                    count,
                    from.display_name(),
                    to.display_name()
                );
            }
            Ok(())
        }

        Command::Styles { lang } => {
            let profiles = match lang {
                Some(tag) => register::profiles_for(&Language::new(tag)),
                None => register::builtin_profiles(),
            };
            for profile in profiles {
                println!(
                    "{:<18} {:<8} {}",
                    profile.id, profile.language, profile.description
                );
                if let Some(voice) = profile.voices.first() {
                    let p = &voice.pronouns;
                    if !p.first_singular.is_empty() || !p.second_singular.is_empty() {
                        println!(
                            "                   I: {:?}  you: {:?}",
                            p.first_singular, p.second_singular
                        );
                    }
                }
            }
            Ok(())
        }

        Command::Localize {
            jar,
            into,
            name,
            target,
            style,
            source_language,
            no_branding,
        } => {
            let mut project = import(&jar, into, name, &target, source_language)?;
            for t in project.profile_mut().targets.iter_mut() {
                t.style_profile = style.clone();
            }
            if no_branding {
                project.profile_mut().branding = Branding {
                    enabled: false,
                    ..Branding::default()
                };
            }
            project.save()?;
            println!("project {}", project.root().display());

            let manifest = project.analyze()?;
            println!("{} capabilities detected", manifest.capabilities.len());

            let graph = project.extract()?;
            report_graph(&graph);

            let mut failed = false;
            let languages: Vec<Language> = project
                .active_targets()
                .iter()
                .map(|t| t.language.clone())
                .collect();
            for language in languages {
                println!("== {}", language.display_name());
                let set = project.suggest(&language, 0.75)?;
                let mut approved = project.translations(&language)?;
                let applied = suggest::apply_safe(&set, &mut approved);
                project.save_translations(&language, &approved)?;
                println!(
                    "{} candidates, {applied} approved from memory and locked glossary terms",
                    set.candidates.len()
                );

                let record = project.build(&language)?;
                report_build(&project, &record);
                report_validation(&record.validation);
                failed |= !record.validation.is_ok();

                // A first run over a game with no memory behind it approves almost nothing, so it
                // produces a working but largely untranslated archive. Saying so beats a "done"
                // that sounds like the game is in the target language.
                let remaining = graph.translatable().count() - approved.len();
                if remaining > 0 {
                    println!(
                        "{remaining} strings still need a human translation - see {}/translations/",
                        project.root().display()
                    );
                }
            }
            if failed {
                bail!("at least one build did not pass validation");
            }
            Ok(())
        }
    }
}

/// The languages a command should act on.
fn languages(project: &Project, lang: Option<&str>, all: bool) -> Result<Vec<Language>> {
    if all {
        let languages: Vec<Language> = project
            .active_targets()
            .iter()
            .map(|t| t.language.clone())
            .collect();
        if languages.is_empty() {
            bail!("this project has no enabled targets");
        }
        return Ok(languages);
    }
    Ok(vec![one_language(project, lang)?])
}

/// One language: the one asked for, or the project's first target.
fn one_language(project: &Project, lang: Option<&str>) -> Result<Language> {
    match lang {
        Some(tag) => {
            let language = Language::new(tag);
            if project.target(&language).is_none() {
                bail!(
                    "this project has no {language} target - add it with `tjlocalizer targets <project> --add {language}`"
                );
            }
            Ok(language)
        }
        None => project
            .active_targets()
            .first()
            .map(|t| t.language.clone())
            .context("this project has no enabled targets"),
    }
}

/// The register profile to start a language with.
fn default_style(language: &Language) -> String {
    register::profiles_for(language)
        .first()
        .map(|p| p.id.clone())
        .unwrap_or_else(|| format!("{}-plain", language.base()))
}

fn import(
    jar: &Path,
    into: Option<PathBuf>,
    name: Option<String>,
    targets: &[String],
    source_language: Option<String>,
) -> Result<Project> {
    let bytes = std::fs::read(jar).with_context(|| format!("cannot read {}", jar.display()))?;
    let name = name.unwrap_or_else(|| {
        jar.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "game".to_string())
    });
    let root = into.unwrap_or_else(|| PathBuf::from("projects").join(&name));
    let mut project = Project::create(&root, &name, &bytes)?;

    if let Some(tag) = source_language {
        let profile = project.profile_mut();
        profile.source_language.language = Language::new(tag);
        profile.source_language.detected = false;
    }
    if !targets.is_empty() {
        let chosen: Vec<Target> = targets
            .iter()
            .map(|spec| match spec.split_once(':') {
                Some((tag, style)) => Target::new(Language::new(tag), style),
                None => {
                    let language = Language::new(spec.as_str());
                    let style = default_style(&language);
                    Target::new(language, style)
                }
            })
            .collect();
        project.profile_mut().targets = chosen;
    }
    project.save()?;
    Ok(project)
}

/// Shows what the offline dictionary engine makes of the strings nobody has translated.
///
/// Printed as glosses, never as translations: the engine resolves terms, and a stitched-together
/// gloss of a sentence is a starting point for a person, not an answer.
fn show_glosses(project: &Project, language: &Language) -> Result<()> {
    let graph = project.graph()?;
    let approved = project.translations(language)?;
    let dictionary = project.dictionary()?;
    let glossary = project.glossary(language)?;
    let memory = project.memory(language)?;
    let target = project
        .target(language)
        .context("the project has no such target")?;
    let style = register::builtin(&target.style_profile);

    let mut provider = DictionaryProvider::new(&dictionary, &glossary);
    if let Some(style) = style.as_ref() {
        provider = provider.with_style(style);
    }

    let mut complete = 0usize;
    let mut partial = 0usize;
    let mut shown = 0usize;

    for node in graph.translatable() {
        if approved.get(&node.id).is_some() {
            continue;
        }
        let request = Request {
            source_text: node.source_text.clone(),
            from: project.source_language().clone(),
            to: language.clone(),
            context: format!("{:?}", node.context).to_lowercase(),
            placeholders: node.constraints.placeholders.clone(),
            speaker: Default::default(),
            stance: Default::default(),
        };
        let Some(proposal) = translate::propose(&request, &memory, &[&provider]) else {
            continue;
        };
        match proposal.completeness {
            Completeness::Complete => complete += 1,
            Completeness::Partial => partial += 1,
            Completeness::None => continue,
        }
        if shown < 12 {
            let mark = match proposal.completeness {
                Completeness::Complete => "full",
                _ => "part",
            };
            println!(
                "  [gloss {mark} {:.2}] {:?} -> {:?}",
                proposal.confidence, node.source_text, proposal.target_text
            );
            shown += 1;
        }
    }
    println!(
        "dictionary glosses: {complete} fully resolved, {partial} partial - all need a person, \
         none are approved automatically"
    );
    Ok(())
}

fn read_archive(path: &Path) -> Result<Archive> {
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(Archive::read(&bytes)?)
}

fn report_graph(graph: &ContentGraph) {
    let translatable = graph.translatable().count();
    println!(
        "{} text nodes, {translatable} translatable",
        graph.nodes.len()
    );
    let mut by_context: std::collections::BTreeMap<String, usize> = Default::default();
    for node in graph.translatable() {
        *by_context.entry(format!("{:?}", node.context)).or_default() += 1;
    }
    for (context, count) in by_context {
        println!("  {context:<10} {count}");
    }
}

fn report_build(project: &Project, record: &BuildRecord) {
    let name = project
        .target(&record.language)
        .map(|t| project.output_name(t))
        .unwrap_or_default();
    println!(
        "build {:04} [{}]: {} literals in {} classes, {} resources",
        record.revision,
        record.language,
        record.report.literals_patched,
        record.report.classes_patched,
        record.report.resources_patched
    );
    println!("  output/{name}");
    println!("  sha256 {}", record.report.output_sha256);
}

fn report_validation(report: &ValidationReport) {
    if report.findings.is_empty() {
        println!("validation: clean");
        return;
    }
    for finding in &report.findings {
        let label = match finding.severity {
            Severity::Error => "error",
            Severity::Warning => "warn ",
        };
        println!("  {label} {:<22} {}", finding.check, finding.detail);
    }
}
