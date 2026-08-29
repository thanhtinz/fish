//! Command line front end (specification §27).
//!
//! Each subcommand is one step of the workflow in §22 and leaves its result on disk, so the steps
//! can be run separately, by different people, days apart. `localize` runs the whole chain for the
//! case where nobody needs to intervene.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use tjlocalizer_core::build::Branding;
use tjlocalizer_core::graph::ContentGraph;
use tjlocalizer_core::jar::Archive;
use tjlocalizer_core::project::Project;
use tjlocalizer_core::suggest::{self, Origin};
use tjlocalizer_core::validate::{inspect, Severity, ValidationReport};

#[derive(Parser)]
#[command(
    name = "tjlocalizer",
    version,
    about = "Thanhtinz JAR Localizer - Java/J2ME game localization",
    long_about = "Localizes Java and J2ME game archives. Nothing here is specific to any one \
                  game: what a game supports is detected, and what is done about it comes from \
                  the project's rules, glossary and memory."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Import a JAR into a new project directory.
    Import {
        /// The game archive. Treated as untrusted input.
        jar: PathBuf,
        /// Where to create the project. Defaults to projects/<name>.
        #[arg(long)]
        into: Option<PathBuf>,
        /// Project name. Defaults to the JAR's file stem.
        #[arg(long)]
        name: Option<String>,
        /// Target language tag.
        #[arg(long, default_value = "vi-VN")]
        target: String,
    },

    /// Detect what the game supports, and write the capability manifest.
    Analyze { project: PathBuf },

    /// Extract translatable text into the content graph.
    Extract { project: PathBuf },

    /// Propose translations from the project's memory and glossary.
    Translate {
        project: PathBuf,
        /// Similarity below which a memory entry is not worth proposing.
        #[arg(long, default_value_t = 0.75)]
        fuzzy_threshold: f32,
        /// Approve the candidates that merely restate a decision this project already made:
        /// exact memory hits and locked glossary terms. Fuzzy matches are never approved.
        #[arg(long)]
        apply_safe: bool,
    },

    /// Apply approved translations and repackage.
    Build {
        project: PathBuf,
        /// Build without the Thanhtinz localization attribution.
        #[arg(long)]
        no_branding: bool,
    },

    /// Check a project's latest build, or a JAR on its own.
    Validate { target: PathBuf },

    /// Report whether a built archive is ready to launch.
    Test { jar: PathBuf },

    /// List a project's recorded builds.
    Builds { project: PathBuf },

    /// Restore an earlier build as the current output.
    Rollback { project: PathBuf, revision: u32 },

    /// Import, analyze, extract, propose, build and validate in one pass.
    Localize {
        jar: PathBuf,
        #[arg(long)]
        into: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, default_value = "vi-VN")]
        target: String,
        /// Register style, recorded in the project profile.
        #[arg(long, default_value = "natural-dialogue")]
        style: String,
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
        } => {
            let project = import(&jar, into, name, &target)?;
            println!(
                "imported {} into {}",
                jar.display(),
                project.root().display()
            );
            println!("  sha256 {}", project.profile().source.sha256);
            println!(
                "  target {}",
                project.profile().localization.target_language
            );
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

        Command::Translate {
            project,
            fuzzy_threshold,
            apply_safe,
        } => {
            let project = Project::open(&project)?;
            let set = project.suggest(fuzzy_threshold)?;
            println!(
                "{} candidates, {} nodes with nothing to propose",
                set.candidates.len(),
                set.without_candidate
            );
            for candidate in set.candidates.iter().take(20) {
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
            if set.candidates.len() > 20 {
                println!(
                    "  ... {} more in translations/candidates.json",
                    set.candidates.len() - 20
                );
            }

            if apply_safe {
                let mut approved = project.translations()?;
                let applied = suggest::apply_safe(&set, &mut approved);
                project.save_translations(&approved)?;
                println!("approved {applied} candidates that restate an existing decision");
            }
            Ok(())
        }

        Command::Build {
            project,
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
            let record = project.build()?;
            report_build(&project, &record);
            if !record.validation.is_ok() {
                bail!("the build did not pass validation");
            }
            Ok(())
        }

        Command::Validate { target } => {
            let report = if target.join("project.json").exists() {
                let project = Project::open(&target)?;
                match project.builds()?.pop() {
                    Some(record) => record.validation,
                    None => bail!("{} has no build to validate", target.display()),
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
            // Said plainly rather than implied by a green result: passing these checks means the
            // archive is well formed and declares an entry point that exists, not that the game
            // was played through. Emulator and screenshot regression (§25) is not implemented.
            println!("note: static checks only - emulator regression is not implemented yet");
            if !report.is_ok() {
                bail!("the archive is not ready to launch");
            }
            Ok(())
        }

        Command::Builds { project } => {
            let project = Project::open(&project)?;
            let builds = project.builds()?;
            if builds.is_empty() {
                println!("no builds recorded");
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
            Ok(())
        }

        Command::Rollback { project, revision } => {
            let project = Project::open(&project)?;
            let record = project.rollback(revision)?;
            println!(
                "restored build {:04} to output/{}",
                record.revision,
                project.output_name()
            );
            Ok(())
        }

        Command::Localize {
            jar,
            into,
            name,
            target,
            style,
            no_branding,
        } => {
            let mut project = import(&jar, into, name, &target)?;
            project.profile_mut().localization.style_profile = style;
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

            let set = project.suggest(0.75)?;
            let mut approved = project.translations()?;
            let applied = suggest::apply_safe(&set, &mut approved);
            project.save_translations(&approved)?;
            println!(
                "{} candidates, {applied} approved from memory and locked glossary terms",
                set.candidates.len()
            );

            let record = project.build()?;
            report_build(&project, &record);
            report_validation(&record.validation);

            // A one-shot run over a game with no memory behind it approves almost nothing, so it
            // produces a working but largely untranslated archive. Saying so beats a "done" that
            // sounds like the game is in Vietnamese.
            let remaining = graph.translatable().count() - approved.len();
            if remaining > 0 {
                println!(
                    "{remaining} strings still need a human translation - see {}/translations/",
                    project.root().display()
                );
            }
            if !record.validation.is_ok() {
                bail!("the build did not pass validation");
            }
            Ok(())
        }
    }
}

fn import(
    jar: &Path,
    into: Option<PathBuf>,
    name: Option<String>,
    target: &str,
) -> Result<Project> {
    let bytes = std::fs::read(jar).with_context(|| format!("cannot read {}", jar.display()))?;
    let name = name.unwrap_or_else(|| {
        jar.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "game".to_string())
    });
    let root = into.unwrap_or_else(|| PathBuf::from("projects").join(&name));
    let mut project = Project::create(&root, &name, &bytes)?;
    project.profile_mut().localization.target_language = target.to_string();
    project.save()?;
    Ok(project)
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

fn report_build(project: &Project, record: &tjlocalizer_core::project::BuildRecord) {
    println!(
        "build {:04}: {} literals in {} classes, {} resources",
        record.revision,
        record.report.literals_patched,
        record.report.classes_patched,
        record.report.resources_patched
    );
    println!("  output/{}", project.output_name());
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
