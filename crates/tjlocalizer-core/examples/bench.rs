//! Times the pipeline against a game far larger than the ones it was written for (§33).
//!
//! Not a microbenchmark. What matters for this tool is whether a person waits: opening a project,
//! extracting its text, proposing translations for all of it, building, and validating. Each of
//! those is timed here on a synthetic game whose size is given on the command line, so a change
//! that makes one of them quadratic shows up as a number rather than as a complaint months later.
//!
//! Synthetic rather than a real game, because a real one cannot be committed and a benchmark
//! nobody can run is a number nobody can check.

use std::time::Instant;
use tjlocalizer_core::build::{apply, Branding};
use tjlocalizer_core::dictionary_data;
use tjlocalizer_core::graph;
use tjlocalizer_core::jar::Archive;
use tjlocalizer_core::lang::Language;
use tjlocalizer_core::translate::{propose, Request};
use tjlocalizer_core::translation::{Glossary, TranslationMemory, TranslationStore};
use tjlocalizer_core::validate::{validate, Subject};

fn main() -> anyhow::Result<()> {
    let resources: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(200);
    let per_resource: usize = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(100);

    let class = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/SampleGame.class"
    ))?;

    let mut archive = Archive::read(&std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/sample-game.jar"
    ))?)?;
    for i in 0..resources {
        let mut text = String::new();
        for key in 0..per_resource {
            // Deliberately repetitive: a real game says "Attack" on forty screens, and the
            // structures that hold translations have to survive that.
            text.push_str(&format!(
                "screen{i}.item{key}.name=Iron Sword {key}\nscreen{i}.item{key}.hint=Press to \
                 attack the enemy {key}\n"
            ));
        }
        archive.insert(format!("lang/screen{i}.properties"), text.into_bytes());
        if i % 10 == 0 {
            archive.insert(format!("code/Class{i}.class"), class.clone());
        }
    }

    let step = |name: &str, at: Instant| {
        eprintln!("{name:<28} {:>8.0} ms", at.elapsed().as_secs_f64() * 1000.0);
    };

    let at = Instant::now();
    let bytes = archive.write()?;
    step("write the archive", at);
    eprintln!(
        "{:<28} {:>8} entries, {} KiB",
        "game",
        archive.entries().len(),
        bytes.len() / 1024
    );

    let at = Instant::now();
    let archive = Archive::read(&bytes)?;
    step("read the archive", at);

    let at = Instant::now();
    let graph = graph::extract(&archive);
    step("extract", at);
    eprintln!("{:<28} {:>8} nodes", "text", graph.nodes.len());

    let at = Instant::now();
    let inference = tjlocalizer_core::context::infer(&graph);
    step("read the context", at);
    eprintln!(
        "{:<28} {:>8} readings",
        "inferred",
        inference.readings.len()
    );

    let dictionary = dictionary_data::builtin();
    let glossary = Glossary::default();
    let memory = TranslationMemory::default();
    let provider = tjlocalizer_core::translate::DictionaryProvider::new(&dictionary, &glossary);
    let providers: Vec<&dyn tjlocalizer_core::translate::Provider> = vec![&provider];

    let at = Instant::now();
    let mut store = TranslationStore::default();
    let mut proposed = 0usize;
    for node in graph.translatable() {
        let request = Request {
            source_text: node.source_text.clone(),
            from: Language::new("en"),
            to: Language::new("vi-VN"),
            context: node.context.key().to_string(),
            placeholders: node.constraints.placeholders.clone(),
            speaker: Default::default(),
            stance: Default::default(),
        };
        if let Some(proposal) = propose(&request, &memory, &providers) {
            store.set(&node.id, &proposal.target_text);
            proposed += 1;
        }
    }
    step("propose every line", at);
    eprintln!("{:<28} {:>8} proposed", "translations", proposed);

    let at = Instant::now();
    let (built, _) = apply(&archive, &graph, &store, &Branding::default())?;
    step("build", at);

    let at = Instant::now();
    let report = validate(&Subject::new(
        &archive,
        &built,
        &graph,
        &store,
        &Language::new("en"),
        &Language::new("vi-VN"),
    ));
    step("validate", at);
    eprintln!("{:<28} {:>8} findings", "validation", report.findings.len());

    Ok(())
}
