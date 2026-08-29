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
use tjlocalizer_core::claude::{self, Analyst};
use tjlocalizer_core::dictionary::Dictionary;
use tjlocalizer_core::font::outline::MarkSource;
use tjlocalizer_core::font::sheet::Grid;
use tjlocalizer_core::graph::ContentGraph;
use tjlocalizer_core::jar::Archive;
use tjlocalizer_core::lang::Language;
use tjlocalizer_core::project::{BuildRecord, FontProfile, Project, Target};
use tjlocalizer_core::provider::{Briefing, HttpProvider, ProviderConfig, ProviderKind};
use tjlocalizer_core::register;
use tjlocalizer_core::secrets::Keys;
use tjlocalizer_core::suggest::{self, Origin};
use tjlocalizer_core::translate::{self, Completeness, DictionaryProvider, Request};
use tjlocalizer_core::tree;
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
    /// Import a game package into a new project directory.
    ///
    /// A J2ME or Java JAR, an Android APK, an iOS IPA, or a zip of files: all of them are ZIP
    /// archives underneath, and which one it is is worked out from what is inside rather than
    /// from the extension. `analyze` then says what can and cannot be done with it.
    ///
    /// A PC game installed on disk is a directory rather than a file, and that works too: the
    /// tree is walked without opening anything, and only the files in a format this build reads
    /// are copied into the project.
    Import {
        /// The game: a .jar, .apk, .ipa or .zip file, or the directory a PC game is installed
        /// in. A directory is scanned and only the files worth reading are copied in.
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
    Analyze {
        project: PathBuf,
        /// Also ask Claude which files look like they hold text.
        ///
        /// Sends file names, sizes and what the mechanical check made of each. It does not send
        /// any file's contents. What comes back is shown separately and changes nothing.
        #[arg(long)]
        with_claude: bool,
    },

    /// Configure the analysis engine, or show what is configured.
    ///
    /// The same service as the `anthropic` translation engine, so a key stored for one is found
    /// by the other. Off unless switched on, and off means nothing is sent.
    Claude {
        project: PathBuf,
        /// The model to ask. claude-opus-5 by default; claude-haiku-4-5 for large scans.
        #[arg(long)]
        model: Option<String>,
        /// Store the key for this endpoint. Read from TJLOCALIZER_API_KEY when given as `-`.
        #[arg(long)]
        key: Option<String>,
        #[arg(long)]
        enable: bool,
        #[arg(long)]
        disable: bool,
    },

    /// Ask what one file in the package is, and where its text sits.
    ///
    /// The only command that sends any of a game's bytes to a model: the first 2 KiB of the one
    /// entry named, and nothing else.
    Inspect {
        project: PathBuf,
        /// The archive entry to ask about.
        entry: String,
        /// Print what would be sent, and send nothing.
        #[arg(long)]
        dry_run: bool,
    },

    /// Ask Claude to read the approved translations and say what looks wrong.
    ///
    /// This sends the game's own text and its translation. It is a separate command for that
    /// reason, and it prints how many lines would go before it sends them.
    Review {
        project: PathBuf,
        #[arg(long)]
        lang: Option<String>,
        /// How many lines to send. The rest are left for a later run.
        #[arg(long, default_value_t = 100)]
        limit: usize,
        /// Print what would be sent, and send nothing.
        #[arg(long)]
        dry_run: bool,
    },

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
        /// Also ask the configured external engine. Sends the untranslated strings to it.
        #[arg(long)]
        engine: bool,
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

    /// Copy a built patch over a game installed on disk.
    ///
    /// Only for a game imported as a directory. Every file is checked against the version the
    /// patch was built from before anything is written, and what it replaces is kept - so this
    /// can be undone, and a patch built for a different copy of the game is refused whole.
    ApplyPatch {
        project: PathBuf,
        /// The game directory to write into.
        #[arg(long)]
        to: PathBuf,
        #[arg(long)]
        lang: Option<String>,
        /// Show what would be overwritten and write nothing.
        #[arg(long)]
        dry_run: bool,
    },

    /// Show the dictionaries available, and what they cover.
    Dictionaries {
        /// A project, to include its own packs. Omit for the built-in ones only.
        project: Option<PathBuf>,
    },

    /// Configure the external translation engine, or show what is configured.
    ///
    /// Off unless switched on. When on, the game's text is sent to whichever service is
    /// configured; that is the user's decision and their key.
    Engine {
        project: PathBuf,
        /// openai-compatible, deepl, google-v2, libretranslate or anthropic.
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        model: Option<String>,
        /// Store the key for this endpoint. Read from TJLOCALIZER_API_KEY when given as `-`.
        #[arg(long)]
        key: Option<String>,
        /// Switch the engine on. Nothing reaches the network until this is done.
        #[arg(long)]
        enable: bool,
        #[arg(long)]
        disable: bool,
        /// Show the exact request that would be sent for a string, without sending it.
        #[arg(long)]
        dry_run: Option<String>,
        #[arg(long)]
        lang: Option<String>,
    },

    /// Check the game's font against the translations, and compose the letters it lacks.
    ///
    /// A game that draws its own text can only show the glyphs somebody drew into its sheet, and
    /// nobody drew ế. The translation is right, the build passes, and the screen shows blanks.
    Font {
        project: PathBuf,
        #[arg(long)]
        lang: Option<String>,
        /// Declare the archive entry holding the glyph sheet.
        #[arg(long)]
        sheet: Option<String>,
        /// Cell size, as WIDTHxHEIGHT. Required with --sheet: a guessed grid shifts every glyph.
        #[arg(long)]
        cell: Option<String>,
        /// Cells per row.
        #[arg(long)]
        columns: Option<u32>,
        /// Declare that the game uses the handset's own font and needs no sheet.
        #[arg(long)]
        device_font: bool,
        /// Build a sheet with the missing Vietnamese letters composed from the game's own.
        #[arg(long)]
        compose: bool,
        /// A folder of fonts to choose the diacritic shapes from.
        ///
        /// Every font in it is measured against this game's own sheet and the one supplying the
        /// most usable marks wins. Which font that is depends on the cell size and cannot be told
        /// from the file: at twelve pixels a heavy weight contributes twice what an elegant one
        /// does, because its marks survive being rasterised that small.
        #[arg(long)]
        marks_library: Option<PathBuf>,

        /// Render sample text both ways into fonts/preview.png, at the real size and enlarged.
        ///
        /// Whether borrowed marks read better than drawn ones is not something a count answers.
        #[arg(long)]
        preview: bool,

        /// Take the diacritic shapes from this font file rather than drawing them.
        ///
        /// The font is read from where it is and never copied into the project. A borrowed mark
        /// is used only where the letter it makes stays unlike every other: at the sizes these
        /// games use, a real typeface's diacritics thin out until a grave and an acute are the
        /// same two pixels.
        #[arg(long)]
        marks_from: Option<PathBuf>,
    },

    /// List the register profiles this build ships.
    Styles {
        /// Only those for a language.
        #[arg(long)]
        lang: Option<String>,
    },

    /// Images with words painted into them (§17).
    ///
    /// Each image is listed with what its shape suggests, a person decides, and what they decide
    /// is written down - after which the build reports every marked image that still ships its
    /// original artwork. Where the project knows the game's glyph sheet, `--read` matches the
    /// picture against the game's own letters and says what it can read, refusing the shapes it
    /// cannot rather than guessing them.
    Assets {
        project: PathBuf,
        /// Only the images whose shape suggests words.
        #[arg(long)]
        suspect: bool,
        /// Record that this entry carries words.
        #[arg(long)]
        mark: Option<String>,
        /// What it says, for whoever will redraw it.
        #[arg(long)]
        says: Option<String>,
        /// A redrawn version, relative to the project directory.
        #[arg(long)]
        replacement: Option<String>,
        /// Forget an entry that was marked.
        #[arg(long)]
        unmark: Option<String>,
        /// Read the words in the images with the game's own letters. Given an entry, only that
        /// one; otherwise every image whose shape suggests a label.
        #[arg(long, num_args = 0.., value_name = "ENTRY")]
        read: Option<Vec<String>>,
        /// Record what was read as what each image says. Only readings where every shape matched
        /// are taken; the rest are left for a person.
        #[arg(long, requires = "read")]
        accept: bool,
    },

    /// Draw the translations as the game will draw them (§25).
    ///
    /// Not an emulator: no menus, no backgrounds, no buttons. The text itself, in the game's own
    /// glyphs, at the game's own size, with a marker where the original ended - which is where
    /// the failures this tool can see actually live.
    Proof {
        project: PathBuf,
        #[arg(long)]
        lang: Option<String>,
        /// Enlargement, so a twelve-pixel font can be looked at.
        #[arg(long, default_value_t = 4)]
        scale: u32,
    },

    /// What changed in the picture since the last drawing somebody accepted (§25).
    ///
    /// A translator changes six lines and the drawing changes in six places. If it changes in
    /// sixty, something else moved - a font was recomposed, a glyph order edited, a rule installed
    /// a sheet with a different baseline - and that is the class of failure no text report shows.
    Regress {
        project: PathBuf,
        #[arg(long)]
        lang: Option<String>,
        #[arg(long, default_value_t = 4)]
        scale: u32,
        /// Accept the current drawing as what this language should look like from now on.
        #[arg(long)]
        accept: bool,
    },

    /// Runs the build in the emulator this project's owner configured (§25).
    ///
    /// No emulator is shipped, suggested or downloaded. This runs the command written in the
    /// project file, on the newest build - the command is the person's, and nothing read out of
    /// the game can influence it.
    Play {
        project: PathBuf,
        #[arg(long)]
        lang: Option<String>,
        /// Record the program to run, and use it from now on.
        #[arg(long)]
        command: Option<String>,
        /// Its arguments. `{game}` is replaced with the build's path; without it the path is
        /// appended.
        #[arg(long, num_args = 0..)]
        args: Vec<String>,
    },

    /// The per-game patches a project holds, and whether they fit this game (§19).
    ///
    /// A rule is data, not code: it says what it expects to find in the game and what it would
    /// change, and it refuses when the game is not what it was written for. Nothing runs until it
    /// is switched on.
    Rules {
        project: PathBuf,
        /// Write the rule that installs the composed font sheet into the game.
        #[arg(long)]
        install_font: bool,
        /// Switch a rule on by id.
        #[arg(long)]
        enable: Option<String>,
        /// Switch a rule off by id.
        #[arg(long)]
        disable: Option<String>,
        /// Delete a rule by id.
        #[arg(long)]
        remove: Option<String>,
    },

    /// What each line is for and who says it, read from the lines around it (§10, §15).
    ///
    /// `classify` looks at one string on its own, which for `Yes` settles nothing. This looks at
    /// the company a string keeps: the keys beside it, the strings either side of it in the same
    /// class, and whether the line names a speaker. Every reading carries its evidence, and none
    /// of them overrules what the string itself said.
    Context {
        project: PathBuf,
        /// Only the characters found, not the per-line readings.
        #[arg(long)]
        cast: bool,
    },

    /// Adapters for one game or engine, written as data (§20).
    ///
    /// A plugin is a JSON file under the project's `plugins/` directory. It says what to look for
    /// and what to conclude: capabilities to report, files to read as a format this build already
    /// writes, glyph sheets to suggest, rules to offer, terms to add. It is data and only data -
    /// nothing in it is executed - so what it contributes is listed here, against this game, with
    /// what fired and what did not.
    Plugins { project: PathBuf },

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
            println!("  sha256 {}", p.source.sha256());
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

        Command::Analyze {
            project,
            with_claude,
        } => {
            let project = Project::open(&project)?;
            let package = project.package()?;

            println!("{}", package.kind.label());
            for evidence in &package.evidence {
                println!("  {evidence}");
            }
            match package.kind.repackaging_note() {
                Some(note) => println!("  cannot be rebuilt here: {note}"),
                None => println!("  can be rebuilt and packaged here"),
            }

            if !package.readable.is_empty() {
                println!();
                println!("text this build can read:");
                for resource in package.readable.iter().take(20) {
                    println!(
                        "  {:<44} {:<16} {} string{}{}",
                        resource.entry,
                        resource.format,
                        resource.fields,
                        if resource.fields == 1 { "" } else { "s" },
                        if resource.writable {
                            ""
                        } else {
                            "  [read-only]"
                        }
                    );
                }
                if package.readable.len() > 20 {
                    println!("  ... and {} more", package.readable.len() - 20);
                }
            }
            if !package.opaque.is_empty() {
                println!();
                // Named rather than passed over: a translator who cannot see that a game keeps
                // half its dialogue somewhere unreadable will think the game is half translated.
                println!("text this build cannot read:");
                for resource in &package.opaque {
                    println!("  {:<44} {}", resource.entry, resource.reason);
                }
            }
            println!();

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

            if with_claude {
                let settings = claude_settings(&project)?;
                let analyst = Analyst::new(settings.clone());
                let archive = project.original()?;
                let facts = claude::facts(&archive);
                let key = require_key(claude::ENDPOINT)?;

                println!();
                println!(
                    "sending {} file names to {} - names, sizes and formats only, no contents",
                    facts.len(),
                    settings.model
                );
                let (survey, trouble) = analyst.survey_all(&key, &facts);
                for why in &trouble {
                    println!("  a batch did not answer: {why}");
                }

                // Kept apart from what the survey above established, on the page as on disk: a
                // guess and a fact in one list become indistinguishable within a week.
                let mut likely: Vec<_> = survey.verdicts.iter().filter(|v| v.holds_text).collect();
                likely.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
                println!();
                println!("suggested by {} - guesses, not findings:", survey.model);
                if likely.is_empty() {
                    println!("  nothing suggested");
                }
                for verdict in likely {
                    println!(
                        "  {:<44} {:.0}%  {}",
                        verdict.path,
                        verdict.confidence * 100.0,
                        verdict.why
                    );
                }
                project.save_suggestions(&survey)?;
            }
            Ok(())
        }

        Command::Claude {
            project,
            model,
            key,
            enable,
            disable,
        } => {
            let mut project = Project::open(&project)?;
            let mut settings = project.profile().claude.clone().unwrap_or_default();
            if let Some(model) = model {
                settings.model = model;
            }
            if enable {
                settings.enabled = true;
            }
            if disable {
                settings.enabled = false;
            }

            if let Some(key) = key {
                let value = if key == "-" {
                    std::env::var("TJLOCALIZER_API_KEY")
                        .context("TJLOCALIZER_API_KEY is not set")?
                } else {
                    key
                };
                let mut keys = Keys::load(&config_dir());
                keys.set(claude::ENDPOINT, &value);
                keys.save(&config_dir())?;
                println!("key stored for {} (owner-readable only)", claude::ENDPOINT);
            }

            project.profile_mut().claude = Some(settings.clone());
            project.save()?;

            println!("endpoint  {}", claude::ENDPOINT);
            println!("model     {}", settings.model);
            println!(
                "state     {}",
                if settings.enabled {
                    "on - file names will be sent when you ask for a scan"
                } else {
                    "off - nothing leaves this machine"
                }
            );
            println!(
                "key       {}",
                if Keys::load(&config_dir()).has(claude::ENDPOINT) {
                    "stored"
                } else {
                    "not stored"
                }
            );
            Ok(())
        }

        Command::Inspect {
            project,
            entry,
            dry_run,
        } => {
            let project = Project::open(&project)?;
            let archive = project.original()?;
            let found = archive
                .get(&entry)
                .with_context(|| format!("no entry named {entry:?} in this package"))?;
            let sample = claude::Sample::of(&entry, &found.data);
            // A dry run sends nothing, so it works while the engine is off - which is the state a
            // person is in when they want to see what it would send.
            let settings = if dry_run {
                project.profile().claude.clone().unwrap_or_default()
            } else {
                claude_settings(&project)?
            };
            let analyst = Analyst::new(settings.clone());

            if dry_run {
                let call = analyst.inspect_call("<your key>", &sample);
                println!(
                    "would send the first {} bytes of {} and nothing else",
                    claude::Sample::BYTES.min(found.data.len()),
                    entry
                );
                println!("\n--- would POST to {} ---", call.url);
                println!("{}", call.body);
                return Ok(());
            }

            let key = require_key(claude::ENDPOINT)?;
            println!(
                "sending the first {} bytes of {} to {}",
                claude::Sample::BYTES.min(found.data.len()),
                entry,
                settings.model
            );
            let inspection = analyst
                .inspect(&key, &sample)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!();
            println!("format      {}", inspection.format);
            println!("text sits   {}", inspection.where_text_is);
            println!("addressing  {}", inspection.addressing);
            // Printed last and never omitted: this is a guess about a binary format, and a
            // confident wrong one costs somebody a corrupted file.
            println!("caveat      {}", inspection.caveat);
            Ok(())
        }

        Command::Review {
            project,
            lang,
            limit,
            dry_run,
        } => {
            let project = Project::open(&project)?;
            let language = one_language(&project, lang.as_deref())?;
            let graph = project.graph()?;
            let store = project.translations(&language)?;
            let glossary = project.glossary(&language)?;
            let style = project.style(&language);

            let mut lines = Vec::new();
            for node in graph.translatable() {
                // The store holds only what a person approved, so being in it is the filter.
                let Some(target) = store.get(&node.id) else {
                    continue;
                };
                lines.push(claude::ReviewLine {
                    node_id: node.id.clone(),
                    context: node.context.key().to_string(),
                    source: node.source_text.clone(),
                    target: target.to_string(),
                });
                if lines.len() >= limit {
                    break;
                }
            }

            if lines.is_empty() {
                println!("nothing approved to review in {}", language.tag());
                return Ok(());
            }

            // Only the terms that actually occur in what is being sent. The whole glossary would
            // be a larger request saying no more.
            let mut terms: Vec<(String, String)> = Vec::new();
            for line in &lines {
                for term in glossary.matches_in(&line.source) {
                    let pair = (term.source.clone(), term.target.clone());
                    if !terms.contains(&pair) {
                        terms.push(pair);
                    }
                }
            }
            let register = style.as_ref().map(|s| s.description.clone());
            let settings = if dry_run {
                project.profile().claude.clone().unwrap_or_default()
            } else {
                claude_settings(&project)?
            };
            let analyst = Analyst::new(settings.clone());

            // Said before anything goes, not after: this is the one path that sends the game's
            // own text, and the number is what a person needs in order to consent to it.
            println!(
                "{} approved line{} of {} would be sent to {}, with their originals",
                lines.len(),
                if lines.len() == 1 { "" } else { "s" },
                language.tag(),
                settings.model
            );

            if dry_run {
                let call = analyst.review_call("<your key>", &lines, register.as_deref(), &terms);
                println!("\n--- would POST to {} ---", call.url);
                println!("{}", call.body);
                return Ok(());
            }

            let key = require_key(claude::ENDPOINT)?;
            let notes = analyst
                .review(&key, &lines, register.as_deref(), &terms)
                .map_err(|e| anyhow::anyhow!(e))?;
            println!();
            if notes.is_empty() {
                println!("nothing flagged");
            }
            for note in &notes {
                println!("  [{}] {}", note.kind, note.node_id);
                println!("      {}", note.detail);
                if !note.suggestion.is_empty() {
                    // A suggestion, printed as one. Nothing here writes to the store.
                    println!("      suggested: {}", note.suggestion);
                }
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
            engine,
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

                if gloss || engine {
                    show_proposals(&project, &language, engine)?;
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
                // Printed here, not only on `validate`: a build that says it failed without
                // saying why sends the reader looking for a second command to run.
                if !record.validation.findings.is_empty() {
                    report_validation(&record.validation);
                }
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

        Command::ApplyPatch {
            project,
            to,
            lang,
            dry_run,
        } => {
            let project = Project::open(&project)?;
            let language = one_language(&project, lang.as_deref())?;
            let plan = project.plan_patch(&language, &to)?;

            println!("{} in {}", to.display(), language.tag());
            for change in &plan.ready {
                println!("  {}", change.path);
            }
            for bad in &plan.mismatched {
                println!("  {}  {}", bad.path, bad.reason);
            }

            if dry_run {
                println!();
                println!(
                    "{} file{} would be overwritten; nothing was written",
                    plan.ready.len(),
                    if plan.ready.len() == 1 { "" } else { "s" }
                );
                return Ok(());
            }
            if !plan.is_applicable() {
                bail!("the patch does not fit this copy of the game, so none of it was applied");
            }

            let written = project.apply_patch(&language, &to)?;
            println!();
            println!(
                "wrote {} file{}; what they replaced is kept under builds/",
                written.len(),
                if written.len() == 1 { "" } else { "s" }
            );
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

        Command::Engine {
            project,
            kind,
            endpoint,
            model,
            key,
            enable,
            disable,
            dry_run,
            lang,
        } => {
            let mut project = Project::open(&project)?;
            let mut config = project
                .profile()
                .provider
                .clone()
                .unwrap_or_else(ProviderConfig::default);

            if let Some(kind) = kind {
                // Found by walking the list rather than by matching literals: a family added to
                // `all()` used to appear in the application and not here, which is a difference
                // nobody would think to look for.
                config.kind = match ProviderKind::all().into_iter().find(|k| k.id() == kind) {
                    Some(kind) => kind,
                    None => bail!(
                        "unknown engine {kind:?}; try one of: {}",
                        ProviderKind::all()
                            .iter()
                            .map(|k| k.id())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                };
                config.endpoint = config.kind.default_endpoint().to_string();
            }
            if let Some(endpoint) = endpoint {
                config.endpoint = endpoint;
            }
            if let Some(model) = model {
                config.model = Some(model);
            }
            if enable {
                config.enabled = true;
            }
            if disable {
                config.enabled = false;
            }

            if let Some(key) = key {
                // "-" means read it from the environment, so a key never has to appear in shell
                // history or in a process listing.
                let value = if key == "-" {
                    std::env::var("TJLOCALIZER_API_KEY")
                        .context("TJLOCALIZER_API_KEY is not set")?
                } else {
                    key
                };
                let mut keys = Keys::load(&config_dir());
                keys.set(&config.endpoint, &value);
                keys.save(&config_dir())?;
                println!("key stored for {} (owner-readable only)", config.endpoint);
            }

            project.profile_mut().provider = Some(config.clone());
            project.save()?;

            println!("engine    {}", config.kind.id());
            println!("endpoint  {}", config.endpoint);
            if let Some(model) = &config.model {
                println!("model     {model}");
            }
            println!(
                "state     {}",
                if config.enabled {
                    "on - the game's text will be sent to this service"
                } else {
                    "off - nothing leaves this machine"
                }
            );
            println!(
                "key       {}",
                if Keys::load(&config_dir()).has(&config.endpoint) {
                    "stored"
                } else {
                    "not stored"
                }
            );

            if let Some(text) = dry_run {
                let language = one_language(&project, lang.as_deref())?;
                let glossary = project.glossary(&language)?;
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
                println!("\n--- would POST to {} ---", call.url);
                println!("{}", call.body);
            }
            Ok(())
        }

        Command::Font {
            project,
            lang,
            sheet,
            cell,
            columns,
            device_font,
            compose,
            marks_from,
            marks_library,
            preview,
        } => {
            let mut project = Project::open(&project)?;

            if device_font {
                project.profile_mut().font = Some(FontProfile {
                    entry: String::new(),
                    grid: None,
                    order: String::new(),
                    device_font: true,
                    mark_library: None,
                    marks_from: None,
                });
                project.save()?;
            } else if let Some(entry) = sheet {
                let cell = cell.context(
                    "--cell WIDTHxHEIGHT is required with --sheet: a guessed grid shifts every glyph",
                )?;
                let (w, h) = cell.split_once('x').context("--cell must look like 8x12")?;
                let cell_width: u32 = w.parse().context("--cell width")?;
                let cell_height: u32 = h.parse().context("--cell height")?;
                let columns = columns.context("--columns is required with --sheet")?;

                // Rows are worked out from the image, so only the parts a person actually knows
                // have to be given.
                let archive = project.original()?;
                let data = archive
                    .get(&entry)
                    .with_context(|| format!("the archive has no entry {entry}"))?;
                let image = tjlocalizer_core::font::sheet::Image::decode_png(&data.data)?;
                let rows = image.height / cell_height;

                let previous = project.profile().font.clone();
                project.profile_mut().font = Some(FontProfile {
                    entry: entry.clone(),
                    grid: Some(Grid {
                        cell_width,
                        cell_height,
                        columns,
                        rows,
                    }),
                    order: String::new(),
                    device_font: false,
                    // Kept: redeclaring the sheet should not lose the font folder.
                    mark_library: previous.as_ref().and_then(|f| f.mark_library.clone()),
                    marks_from: previous.and_then(|f| f.marks_from),
                });
                project.save()?;
                println!(
                    "{entry}: {}x{} pixels, {columns}x{rows} cells",
                    image.width, image.height
                );
            }

            match project.profile().font.as_ref() {
                None => {
                    println!("no font established for this game");
                    println!(
                        "  if it draws from a glyph sheet, translations will show blanks until one is declared:"
                    );
                    println!(
                        "    tjlocalizer font <project> --sheet font.png --cell 8x12 --columns 16"
                    );
                    println!("  if it uses the handset's own font:");
                    println!("    tjlocalizer font <project> --device-font");
                }
                Some(profile) if profile.device_font => {
                    println!("device font: the handset draws the text, so nothing needs composing")
                }
                Some(profile) => println!("sheet: {}", profile.entry),
            }

            for language in languages(&project, lang.as_deref(), lang.is_none())? {
                if let Some(report) = project.font_report(&language)? {
                    println!("== {language}");
                    println!("  {} glyphs in the sheet", report.covered_count);
                    println!(
                        "  {} of the {} letters Vietnamese needs are missing, {} of them composable from letters the sheet already has",
                        report.missing_required.len(),
                        134,
                        report.composable_count
                    );
                    if report.affected_strings > 0 {
                        println!(
                            "  {} approved translations use {} the font cannot draw: {}",
                            report.affected_strings,
                            report.missing_used.len(),
                            report.missing_used.iter().collect::<String>()
                        );
                    } else {
                        println!("  every approved translation can be drawn");
                    }
                }
            }

            // Measured once and remembered, because scanning a real font folder takes seconds
            // and the answer only changes when the sheet or the folder does.
            if let Some(library) = &marks_library {
                let sheet = project
                    .font_sheet()?
                    .context("declare a glyph sheet first, with --sheet")?;
                println!(
                    "measuring the fonts in {} against this sheet",
                    library.display()
                );
                match tjlocalizer_core::font::library::best_for(&sheet, library)? {
                    None => println!("  none of them supplies a usable mark at this size"),
                    Some(fit) => {
                        println!(
                            "  {} supplies {}/{} marks ({:.0}%)",
                            fit.name,
                            fit.from_typeface,
                            fit.composed,
                            fit.share() * 100.0
                        );
                        if let Some(font) = project.profile_mut().font.as_mut() {
                            font.mark_library = Some(library.clone());
                            font.marks_from = Some(fit.path);
                        }
                        project.save()?;
                    }
                }
            }

            if preview {
                let lines = [
                    "Bat dau tro choi",
                    "Bắt đầu trò chơi",
                    "a ă â á à ả ã ạ ắ ằ ẳ ẵ ặ ấ ầ ẩ ẫ ậ",
                    "e ê é è ẻ ẽ ẹ ế ề ể ễ ệ đ Đ ơ ư",
                ];
                match project.preview_font(&lines, 6)? {
                    None => bail!("declare a glyph sheet first, with --sheet"),
                    Some(path) => {
                        println!("preview written to {}", path.display());
                        println!(
                            "  the drawn marks are on top; if a typeface was chosen its version \
                             follows. Look at the small rows, not the large ones: the small ones \
                             are the size that ships."
                        );
                    }
                }
            }

            if compose {
                let chosen = marks_from.clone().or_else(|| {
                    project
                        .profile()
                        .font
                        .as_ref()
                        .and_then(|f| f.marks_from.clone())
                });
                let marks = match &chosen {
                    Some(path) => {
                        let source = MarkSource::from_path(path)?;
                        if !source.covers_vietnamese() {
                            eprintln!(
                                "warning: {} does not cover every Vietnamese letter; the ones it \
                                 lacks will be drawn instead",
                                path.display()
                            );
                        }
                        Some(source)
                    }
                    None => None,
                };
                match project.compose_font(marks.as_ref())? {
                    None => bail!("declare a glyph sheet first, with --sheet"),
                    Some((path, report)) => {
                        println!(
                            "composed {} glyphs into {}",
                            report.added.len(),
                            path.display()
                        );
                        if let Some(typeface) = &report.typeface {
                            println!(
                                "  {} of {} marks taken from {typeface}; the rest were drawn, \
                                 because a borrowed one would have made two letters identical",
                                report.from_typeface,
                                report.added.len()
                            );
                        }
                        for skipped in report.skipped.iter().take(8) {
                            println!("  skipped {} - {}", skipped.composed, skipped.reason);
                        }
                        if report.skipped.len() > 8 {
                            println!("  ... {} more skipped", report.skipped.len() - 8);
                        }
                        println!(
                            "note: glyphs only. Making the game use them means changing how it \
                             looks characters up, which is per-game and is not done here."
                        );
                    }
                }
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

        Command::Assets {
            project,
            suspect,
            mark,
            says,
            replacement,
            unmark,
            read,
            accept,
        } => {
            let mut project = Project::open(&project)?;

            if let Some(entry) = mark {
                project.mark_text_asset(tjlocalizer_core::assets::TextAsset {
                    entry: entry.clone(),
                    says: says.unwrap_or_default(),
                    replacement: replacement.filter(|r| !r.trim().is_empty()),
                })?;
                println!("{entry} is recorded as carrying words");
            }
            if let Some(entry) = unmark {
                if !project.unmark_text_asset(&entry)? {
                    anyhow::bail!("{entry} was not marked");
                }
                println!("{entry} is no longer marked");
            }

            let marked: Vec<&tjlocalizer_core::assets::TextAsset> =
                project.profile().text_assets.iter().collect();
            if !marked.is_empty() {
                println!("marked as carrying words:");
                for asset in &marked {
                    let state = match &asset.replacement {
                        Some(path) => format!("redrawn as {path}"),
                        None => "nothing to replace it yet".into(),
                    };
                    if asset.says.is_empty() {
                        println!("  {} - {state}", asset.entry);
                    } else {
                        println!("  {} {:?} - {state}", asset.entry, asset.says);
                    }
                }
                println!();
            }

            if let Some(entries) = read {
                let Some(readings) = project.read_text_assets(&entries)? else {
                    anyhow::bail!(
                        "this project has not said which image the game's font is, so there are \
                         no letters to read with: run `tjlocalizer font <project> --candidates`"
                    );
                };
                if readings.is_empty() {
                    println!("nothing to read");
                    return Ok(());
                }
                for reading in &readings {
                    println!(
                        "  {}  {}",
                        reading.entry,
                        if reading.is_complete() {
                            format!("{:?}", reading.text())
                        } else {
                            format!(
                                "{:?} - {} shape{} matched no letter",
                                reading.text(),
                                reading.unread,
                                if reading.unread == 1 { "" } else { "s" }
                            )
                        }
                    );
                    if reading.is_complete() {
                        println!("      every shape matched, worst {:.2}", reading.confidence);
                    }
                }
                if accept {
                    let mut taken = 0usize;
                    for reading in readings.iter().filter(|r| r.is_complete()) {
                        let existing = project
                            .profile()
                            .text_assets
                            .iter()
                            .find(|a| a.entry == reading.entry)
                            .cloned();
                        project.mark_text_asset(tjlocalizer_core::assets::TextAsset {
                            entry: reading.entry.clone(),
                            says: reading.text(),
                            replacement: existing.and_then(|a| a.replacement),
                        })?;
                        taken += 1;
                    }
                    println!();
                    println!("{taken} recorded as carrying those words");
                }
                return Ok(());
            }

            let assets = project.image_assets()?;
            let shown: Vec<_> = assets
                .iter()
                .filter(|a| !suspect || a.worth_checking())
                .collect();
            if shown.is_empty() {
                println!(
                    "no images{}",
                    if suspect {
                        " that look like labels"
                    } else {
                        ""
                    }
                );
                return Ok(());
            }
            println!("{} of {} images:", shown.len(), assets.len());
            for asset in shown {
                let known = project
                    .profile()
                    .text_assets
                    .iter()
                    .any(|t| t.entry == asset.entry);
                println!(
                    "  {}{}  {}x{}, {} colour{}",
                    asset.entry,
                    if known { " [marked]" } else { "" },
                    asset.width,
                    asset.height,
                    asset.colours,
                    if asset.colours == 1 { "" } else { "s" }
                );
                for hint in &asset.hints {
                    println!("      {}", describe_hint(hint));
                }
            }
            println!();
            println!("Read them with the game's own letters, or mark them by hand:");
            println!("  tjlocalizer assets <project> --read --accept");
            println!("  tjlocalizer assets <project> --mark <entry> --says \"START\"");
            Ok(())
        }

        Command::Proof {
            project,
            lang,
            scale,
        } => {
            let project = Project::open(&project)?;
            let language = one_language(&project, lang.as_deref())?;

            match project.proof_sheet(&language, scale)? {
                Some(path) => {
                    println!("{}", path.display());
                    println!(
                        "  the original above, the translation below, and a line where the \n  \
                         original ended - anything past it may not fit"
                    );
                }
                None => println!(
                    "nothing to draw: this needs a declared glyph sheet and at least one approved \n\
                     translation"
                ),
            }
            Ok(())
        }

        Command::Context { project, cast } => {
            let project = Project::open(&project)?;
            let inference = project.infer_context()?;

            if !inference.cast.is_empty() {
                println!("{} named character(s):", inference.cast.len());
                for character in &inference.cast {
                    println!(
                        "  {} - {} line{} in {}",
                        character.name,
                        character.lines,
                        if character.lines == 1 { "" } else { "s" },
                        character.appears_in.join(", ")
                    );
                    if !character.beside.is_empty() {
                        println!("      named beside {}", character.beside.join(", "));
                    }
                    if let Some(hint) = &character.suggested_stance {
                        println!(
                            "      sounds {:?} - {} - nothing applies this; it is for a person to judge",
                            hint.stance,
                            hint.because.join(", ")
                        );
                    }
                }
                println!();
            }
            if cast {
                return Ok(());
            }

            if inference.readings.is_empty() {
                println!("nothing to add: every string spoke for itself.");
                return Ok(());
            }

            let graph = project.graph()?;
            println!("{} reading(s):", inference.readings.len());
            for reading in &inference.readings {
                let text = graph
                    .get(&reading.node)
                    .map(|n| n.source_text.as_str())
                    .unwrap_or("");
                println!("  {:?}", text);
                if let Some(context) = reading.context {
                    println!("      reads as {}", context.key());
                }
                if let Some(character) = &reading.character {
                    println!("      said by {character}");
                }
                for why in &reading.why {
                    println!("      because {why}");
                }
            }
            Ok(())
        }

        Command::Plugins { project } => {
            let project = Project::open(&project)?;
            let plugins = project.plugins()?;

            if plugins.is_empty() {
                println!("no plugins.");
                println!();
                println!(
                    "A plugin is a JSON file in {}/plugins/ naming what to look for in this",
                    project.root().display()
                );
                println!("game and what to conclude. See docs/PLUGINS.md for the shape of one.");
                return Ok(());
            }

            for (path, reason) in &plugins.broken {
                println!("{} could not be read: {reason}", path.display());
            }

            let archive = project.original()?;
            let fired = plugins.capabilities(&archive);
            let claimed: Vec<String> = archive
                .entries()
                .iter()
                .filter(|e| plugins.formats().of(&e.name).is_some())
                .map(|e| e.name.clone())
                .collect();

            for plugin in &plugins.loaded {
                println!("{}", plugin.id);
                if !plugin.description.is_empty() {
                    println!("  {}", plugin.description);
                }
                if !plugin.author.is_empty() {
                    println!("  written by {}", plugin.author);
                }
                for problem in plugin.problems() {
                    println!("  broken: {problem}");
                }
                for rule in &plugin.capabilities {
                    let holds = fired.iter().any(|c| c.id == rule.id);
                    println!(
                        "  capability {} - {}",
                        rule.id,
                        if holds {
                            "matches this game"
                        } else {
                            "does not match this game"
                        }
                    );
                }
                for resource in &plugin.resources {
                    let hits = archive
                        .entries()
                        .iter()
                        .filter(|e| tjlocalizer_core::plugin::glob(&resource.pattern, &e.name))
                        .count();
                    println!(
                        "  resource {} as {} - {hits} file{} in this game",
                        resource.pattern,
                        resource.format,
                        if hits == 1 { "" } else { "s" }
                    );
                }
                for font in &plugin.fonts {
                    println!(
                        "  font {} - {}x{} cells, {} columns",
                        font.pattern, font.cell_width, font.cell_height, font.columns
                    );
                }
                for rule in &plugin.rules {
                    println!(
                        "  rule {}:{} - offered, off until switched on",
                        plugin.id, rule.id
                    );
                }
                if let Some(pack) = &plugin.dictionary {
                    println!(
                        "  dictionary {} entries, {} to {}",
                        pack.entries.len(),
                        pack.from.tag(),
                        pack.to.tag()
                    );
                }
            }

            println!();
            println!(
                "{} capabilit{} fired, {} file{} claimed as text",
                fired.len(),
                if fired.len() == 1 { "y" } else { "ies" },
                claimed.len(),
                if claimed.len() == 1 { "" } else { "s" }
            );
            for entry in claimed.iter().take(20) {
                println!("  {entry}");
            }
            if claimed.len() > 20 {
                println!("  ... and {} more", claimed.len() - 20);
            }
            Ok(())
        }

        Command::Regress {
            project,
            lang,
            scale,
            accept,
        } => {
            let project = Project::open(&project)?;
            let language = one_language(&project, lang.as_deref())?;

            if accept {
                match project.accept_baseline(&language, scale)? {
                    Some(path) => {
                        println!(
                            "accepted {} as the drawing to compare against",
                            path.display()
                        );
                        return Ok(());
                    }
                    None => anyhow::bail!(
                        "nothing to draw: this needs a declared glyph sheet and at least one \
                         approved translation"
                    ),
                }
            }

            match project.visual_regression(&language, scale)? {
                None => {
                    println!(
                        "nothing to compare against yet. Look at `tjlocalizer proof`, and when it \
                         is right:"
                    );
                    println!("  tjlocalizer regress <project> --accept");
                }
                Some((difference, marked)) if difference.is_identical() => {
                    println!("the drawing is identical to the one accepted.");
                    println!("  {}", marked.display());
                }
                Some((difference, marked)) => {
                    if difference.resized {
                        println!(
                            "the drawing changed size: {}x{} became {}x{}",
                            difference.before.0,
                            difference.before.1,
                            difference.after.0,
                            difference.after.1
                        );
                    }
                    println!(
                        "{} pixel{} changed, {:.2}% of the picture, in {} place{}:",
                        difference.changed,
                        if difference.changed == 1 { "" } else { "s" },
                        difference.share() * 100.0,
                        difference.bands.len(),
                        if difference.bands.len() == 1 { "" } else { "s" }
                    );
                    for band in difference.bands.iter().take(20) {
                        println!(
                            "  rows {}-{}, {} pixels",
                            band.top, band.bottom, band.changed
                        );
                    }
                    if difference.bands.len() > 20 {
                        println!("  ... and {} more", difference.bands.len() - 20);
                    }
                    println!("  {}", marked.display());
                    println!("  Accept it with --accept once it is what you meant.");
                }
            }
            Ok(())
        }

        Command::Play {
            project,
            lang,
            command,
            args,
        } => {
            let mut project = Project::open(&project)?;
            if let Some(command) = command {
                project.profile_mut().emulator =
                    Some(tjlocalizer_core::regress::Emulator { command, args });
                project.save()?;
                println!("recorded the emulator; it will be used from now on");
            }
            let language = one_language(&project, lang.as_deref())?;
            let status = project.play(&language)?;
            if !status.success() {
                anyhow::bail!("the emulator exited with {status}");
            }
            Ok(())
        }

        Command::Rules {
            project,
            install_font,
            enable,
            disable,
            remove,
        } => {
            let project = Project::open(&project)?;

            if install_font {
                let rule = project.font_install_rule()?;
                let id = rule.id.clone();
                let actions = rule.then.len();
                project.put_rule(rule)?;
                println!("wrote rule {id}, switched off");
                println!("  it replaces the sheet.");

                let candidates = project.font_lookup_candidates()?;
                if candidates.is_empty() {
                    println!(
                        "  Nothing in this game looked like where the sheet's shape is written \
                         down,"
                    );
                    println!(
                        "  so making it read the new rows is left for a person: add it as \
                         setIntConstant"
                    );
                    println!("  or setStringConstant in rules/rules.json, then enable it.");
                } else {
                    println!(
                        "  These look like the game's own record of the sheet's shape - what was \
                         found,"
                    );
                    println!("  not what was verified:");
                    for candidate in &candidates {
                        println!(
                            "    {} holds {} as {}",
                            candidate.class,
                            if candidate.value.chars().count() > 24 {
                                format!(
                                    "{:?}…",
                                    &candidate.value.chars().take(24).collect::<String>()
                                )
                            } else {
                                format!("{:?}", candidate.value)
                            },
                            candidate.what.key()
                        );
                    }
                    if actions > 1 {
                        println!(
                            "  {} of them are in the rule as proposed changes. Read them against \
                             the game",
                            actions - 1
                        );
                        println!("  you know, delete what does not belong, then enable it.");
                    }
                }
            }
            for (id, state) in [(enable, true), (disable, false)] {
                if let Some(id) = id {
                    if !project.set_rule_enabled(&id, state)? {
                        anyhow::bail!("this project has no rule {id}");
                    }
                    println!("{id} is now {}", if state { "on" } else { "off" });
                }
            }
            if let Some(id) = remove {
                if !project.remove_rule(&id)? {
                    anyhow::bail!("this project has no rule {id}");
                }
                println!("removed {id}");
            }

            let plans = project.plan_rules()?;
            if plans.is_empty() {
                println!("no rules. `--install-font` writes the one this tool can generate.");
                return Ok(());
            }
            for plan in &plans {
                let state = if plan.ready() {
                    "on, fits"
                } else if plan.enabled {
                    "on, does not fit"
                } else {
                    "off"
                };
                println!("{} [{state}]", plan.id);
                if !plan.description.is_empty() {
                    println!("  {}", plan.description);
                }
                for effect in &plan.effects {
                    println!("  would {effect}");
                }
                for reason in &plan.unmet {
                    println!("  blocked: {reason}");
                }
                if plan.effects.is_empty() && plan.unmet.is_empty() {
                    println!("  nothing to do: it matches nothing in this game");
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
    // A game is either one file or a directory, and which it is decides everything below. For a
    // directory the name comes from the folder, which is what a person would have called it.
    let name = name.unwrap_or_else(|| {
        jar.file_name()
            .map(|s| {
                let full = s.to_string_lossy().to_string();
                if jar.is_dir() {
                    full
                } else {
                    jar.file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or(full)
                }
            })
            .unwrap_or_else(|| "game".to_string())
    });
    let root = into.unwrap_or_else(|| PathBuf::from("projects").join(&name));

    let mut project = if jar.is_dir() {
        let (project, ingested) =
            Project::create_from_tree(&root, &name, jar, &tree::Limits::default())?;
        report_ingest(&ingested);
        project
    } else {
        let bytes = std::fs::read(jar).with_context(|| format!("cannot read {}", jar.display()))?;
        Project::create(&root, &name, &bytes)?
    };

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

/// What a directory game gave up, before anything else is said about it.
///
/// The two numbers people need are how big the game is and how little of it was read, and they
/// need them in that order: "41 812 files, 23 read" is reassuring, "23 files" on its own sounds
/// like a mistake. Files passed over for a reason are listed individually, because a 300 MB JSON
/// skipped for its size is exactly what somebody needs to know about.
fn report_ingest(ingested: &tree::Ingested) {
    println!(
        "{} files ({}), read {} ({})",
        ingested.scanned,
        human_size(ingested.total_size),
        ingested.files.len(),
        human_size(ingested.files.iter().map(|f| f.size).sum()),
    );
    for line in &ingested.evidence {
        println!("  {line}");
    }
    if !ingested.skipped.is_empty() {
        println!();
        println!("passed over, and worth knowing about:");
        for skipped in &ingested.skipped {
            println!(
                "  {:<44} {:>10}  {}",
                skipped.path,
                human_size(skipped.size),
                skipped.reason
            );
        }
    }
}

fn human_size(bytes: u64) -> String {
    const MIB: u64 = 1024 * 1024;
    if bytes >= MIB {
        format!("{:.1} MB", bytes as f64 / MIB as f64)
    } else if bytes >= 1024 {
        format!("{} kB", bytes / 1024)
    } else {
        format!("{bytes} B")
    }
}

/// Shows what the engines make of the strings nobody has translated.
///
/// Printed as proposals, never as translations. The offline engine resolves terms; an external
/// one writes sentences and is fluent whether or not it is right. Neither is ever approved here.
fn show_proposals(project: &Project, language: &Language, use_engine: bool) -> Result<()> {
    let graph = project.graph()?;
    let approved = project.translations(language)?;
    let dictionary = project.dictionary()?;
    let glossary = project.glossary(language)?;
    let memory = project.memory(language)?;
    let style = project.style(language);

    let mut offline = DictionaryProvider::new(&dictionary, &glossary);
    if let Some(style) = style.as_ref() {
        offline = offline.with_style(style);
    }

    // The external engine is assembled only when asked for, so nothing can reach the network as
    // a side effect of looking at glosses.
    let online = if use_engine {
        let config =
            project.profile().provider.clone().context(
                "no engine is configured - run `tjlocalizer engine <project> --kind ...`",
            )?;
        if !config.enabled {
            bail!("the engine is switched off; pass --enable to `tjlocalizer engine` first");
        }
        let key = Keys::load(&config_dir())
            .get(&config.endpoint)
            .map(str::to_string)
            .or_else(|| std::env::var("TJLOCALIZER_API_KEY").ok())
            .context("no key stored for that endpoint")?;
        eprintln!(
            "sending {} untranslated strings to {} - this leaves your machine",
            graph
                .translatable()
                .filter(|n| approved.get(&n.id).is_none())
                .count(),
            config.endpoint
        );
        Some(HttpProvider::new(
            config,
            key,
            Briefing {
                glossary: &glossary,
                style: style.as_ref(),
            },
        ))
    } else {
        None
    };

    let mut providers: Vec<&dyn tjlocalizer_core::translate::Provider> = Vec::new();
    if let Some(online) = online.as_ref() {
        providers.push(online);
    }
    providers.push(&offline);

    let mut complete = 0usize;
    let mut partial = 0usize;
    let mut refused = 0usize;
    let mut shown = 0usize;

    // Read once for the whole pass: who speaks a line decides its pronouns, and in Vietnamese
    // there is no neutral choice to fall back on.
    let inference = project.inference()?;

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
            speaker: inference.voice(&node.id).0,
            stance: inference.voice(&node.id).1,
        };
        let Some(proposal) = translate::propose(&request, &memory, &providers) else {
            continue;
        };
        match proposal.completeness {
            Completeness::Complete => complete += 1,
            Completeness::Partial => partial += 1,
            Completeness::None => {
                refused += 1;
                if shown < 12 {
                    println!("  [{} refused] {:?}", proposal.engine, node.source_text);
                    for note in &proposal.notes {
                        println!("      {note}");
                    }
                    shown += 1;
                }
                continue;
            }
        }
        if shown < 12 {
            println!(
                "  [{} {:.2}] {:?} -> {:?}",
                proposal.engine, proposal.confidence, node.source_text, proposal.target_text
            );
            for note in &proposal.notes {
                println!("      {note}");
            }
            shown += 1;
        }
    }
    println!(
        "proposals: {complete} complete, {partial} partial, {refused} refused - all need a person, \
         none are approved automatically"
    );
    Ok(())
}

/// Where the application keeps its own settings, including keys.
/// The analysis settings, or a refusal saying how to turn it on.
///
/// Refusing here rather than deeper down means the guarantee is visible at the top of every
/// command: while this is off, no command below it has anything to send.
fn claude_settings(project: &Project) -> Result<claude::Settings> {
    let settings = project.profile().claude.clone().unwrap_or_default();
    if !settings.enabled {
        bail!(
            "the analysis engine is off; turn it on with `tjlocalizer claude <project> --enable`"
        );
    }
    Ok(settings)
}

fn require_key(endpoint: &str) -> Result<String> {
    Keys::load(&config_dir())
        .get(endpoint)
        .map(|k| k.to_string())
        .with_context(|| {
            format!("no key stored for {endpoint}; store one with `tjlocalizer claude <project> --key -`")
        })
}

fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.thanhtinz.tjlocalizer")
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

/// The English wording for one reason an image might carry words.
///
/// The core reports the fact; each interface says it in its own language. This is the command
/// line's, and the application has its own.
fn describe_hint(hint: &tjlocalizer_core::assets::Hint) -> String {
    use tjlocalizer_core::assets::Hint;
    match hint {
        Hint::NameSuggests { word } => format!("its name contains {word:?}"),
        Hint::FewColours {
            colours,
            ink_percent,
        } => format!(
            "{colours} colour{} over {ink_percent}% of the image - lettering rather than a scene",
            if *colours == 1 { "" } else { "s" }
        ),
        Hint::ShapeOfALine {
            width,
            height,
            bands,
        } => format!(
            "{width}x{height} with {bands} band{} of ink - the shape of a line of writing",
            if *bands == 1 { "" } else { "s" }
        ),
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
    // A per-game patch that ran silently would be untraceable: somebody looking at a shipped
    // build has to be able to see that its font was swapped.
    if !record.rules.rules.is_empty() {
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        println!(
            "  rules: {} ({}, {})",
            record.rules.rules.join(", "),
            plural(
                record.rules.entries_replaced,
                "entry replaced",
                "entries replaced"
            ),
            plural(
                record.rules.constants_changed,
                "constant changed",
                "constants changed"
            )
        );
    }
    // A refusal is not a failure and not a success: the build is correct and incomplete, and the
    // incompleteness is invisible from anywhere else.
    for refusal in &record.report.refused {
        if refusal.translations > 0 {
            println!(
                "  left alone: {} ({} translation{} not applied) - {}",
                refusal.resource,
                refusal.translations,
                if refusal.translations == 1 { "" } else { "s" },
                refusal.reason
            );
        }
    }
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
