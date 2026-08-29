//! Adapters written as data (§20, §32).
//!
//! The interesting cases are the boundaries: a plugin makes a file readable that nothing here
//! recognised, and it cannot do anything this build could not already do to any archive.

use tjlocalizer_core::graph::TextSource;
use tjlocalizer_core::jar::Archive;
use tjlocalizer_core::plugin::{glob, Plugins};
use tjlocalizer_core::project::Project;

fn fixture() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/sample-game.jar"
    ))
    .expect("fixture missing - run tools/make-fixtures.sh")
}

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tjlocalizer-plugin-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A game whose text is in a file nothing here would call a resource: a `.txt` holding
/// `key=value` lines, which `resource::detect` reads as anonymous lines because a `.txt` with no
/// section headings is not an INI and not a properties file either.
fn game_with_an_unrecognised_resource() -> Vec<u8> {
    let mut archive = Archive::read(&fixture()).unwrap();
    archive.insert(
        "data/lang/en.txt",
        b"# the game's own file\nmenu.start=Start\nmenu.exit=Exit\n".to_vec(),
    );
    archive.write().unwrap()
}

fn write_plugin(root: &std::path::Path, name: &str, json: serde_json::Value) {
    let dir = root.join("plugins");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_string_pretty(&json).unwrap(),
    )
    .unwrap();
}

#[test]
fn a_plugin_makes_a_file_readable_by_key() {
    let dir = TempDir::new("resource");
    let project =
        Project::create(&dir.0, "sample-game", &game_with_an_unrecognised_resource()).unwrap();

    // Without the plugin the file is still read - every non-blank line is offered - but by line
    // number, which is the reading that does not survive somebody editing the file.
    let before = project.extract().unwrap();
    let keys: Vec<&TextSource> = before
        .nodes
        .iter()
        .map(|n| &n.source)
        .filter(|s| matches!(s, TextSource::ResourceProperty { resource, .. } if resource == "data/lang/en.txt"))
        .collect();
    assert!(keys.is_empty(), "nothing should have keyed this file yet");

    write_plugin(
        &dir.0,
        "engine",
        serde_json::json!({
            "id": "engine",
            "description": "this engine keeps its text in data/lang",
            "resources": [
                { "pattern": "data/lang/*.txt", "format": "properties", "note": "key=value" }
            ]
        }),
    );

    let after = project.extract().unwrap();
    let mut found: Vec<String> = after
        .nodes
        .iter()
        .filter_map(|n| match &n.source {
            TextSource::ResourceProperty { resource, key } if resource == "data/lang/en.txt" => {
                Some(key.clone())
            }
            _ => None,
        })
        .collect();
    found.sort();
    assert_eq!(found, vec!["menu.exit", "menu.start"]);
}

/// What a plugin makes readable, the build has to be able to write. A plugin that opened a file
/// at extraction and not at build time would collect translations nobody could ship.
#[test]
fn what_a_plugin_opens_the_build_writes_back() {
    let dir = TempDir::new("build");
    let project =
        Project::create(&dir.0, "sample-game", &game_with_an_unrecognised_resource()).unwrap();
    write_plugin(
        &dir.0,
        "engine",
        serde_json::json!({
            "id": "engine",
            "resources": [{ "pattern": "data/lang/*.txt", "format": "properties" }]
        }),
    );

    let graph = project.extract().unwrap();
    let node = graph
        .nodes
        .iter()
        .find(|n| {
            matches!(&n.source, TextSource::ResourceProperty { resource, key }
            if resource == "data/lang/en.txt" && key == "menu.start")
        })
        .expect("the plugin should have keyed this file");

    let language = project.profile().targets[0].language.clone();
    let mut store = project.translations(&language).unwrap();
    store.set(&node.id, "Bắt đầu");
    project.save_translations(&language, &store).unwrap();
    project.build(&language).unwrap();

    let name = project.output_name(project.target(&language).unwrap());
    let built = Archive::read(&std::fs::read(dir.0.join("output").join(name)).unwrap()).unwrap();
    let text = String::from_utf8(built.get("data/lang/en.txt").unwrap().data.clone()).unwrap();
    assert!(text.contains("menu.start=Bắt đầu"), "{text}");
    // Everything else survives, as it must for any resource this build edits.
    assert!(text.contains("# the game's own file"), "{text}");
    assert!(text.contains("menu.exit=Exit"), "{text}");
}

#[test]
fn a_plugin_reports_a_capability_with_the_evidence_for_it() {
    let dir = TempDir::new("capability");
    let project =
        Project::create(&dir.0, "sample-game", &game_with_an_unrecognised_resource()).unwrap();
    write_plugin(
        &dir.0,
        "engine",
        serde_json::json!({
            "id": "engine",
            "capabilities": [{
                "id": "some_engine",
                "confidence": 0.8,
                "when": [{ "kind": "entryMatches", "pattern": "data/lang/*" }]
            }]
        }),
    );

    let manifest = project.analyze().unwrap();
    let found = manifest
        .capabilities
        .iter()
        .find(|c| c.id == "some_engine")
        .expect("the capability should have fired");
    assert_eq!(found.confidence, 0.8);
    assert!(
        found.evidence.iter().any(|e| e.contains("plugin engine")),
        "a capability has to name what claimed it: {:?}",
        found.evidence
    );

    // The built-in detectors still run. A plugin adds to what is known; it does not replace it.
    assert!(manifest.capabilities.iter().any(|c| c.id != "some_engine"));
}

/// A capability that matches nothing is not reported, and one that would match everything is
/// refused as broken rather than fired.
#[test]
fn a_capability_that_matches_nothing_is_not_reported() {
    let dir = TempDir::new("no-match");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    write_plugin(
        &dir.0,
        "engine",
        serde_json::json!({
            "id": "engine",
            "capabilities": [
                { "id": "absent", "when": [{ "kind": "entryExists", "entry": "nope.dat" }] },
                { "id": "always", "when": [] }
            ]
        }),
    );

    let manifest = project.analyze().unwrap();
    assert!(!manifest.has("absent"));
    assert!(!manifest.has("always"));

    let problems = project.plugins().unwrap().loaded[0].problems();
    assert!(
        problems.iter().any(|p| p.contains("every game")),
        "{problems:?}"
    );
}

/// A plugin offers rules; it does not run them. Switching one on is a decision this project makes
/// and keeps, which is why it lands in the project's own rules file.
#[test]
fn a_plugin_rule_arrives_switched_off_and_becomes_the_projects_own_when_enabled() {
    let dir = TempDir::new("rules");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    write_plugin(
        &dir.0,
        "engine",
        serde_json::json!({
            "id": "engine",
            "rules": [{
                "id": "widen",
                "description": "the label box in this engine is too narrow",
                "enabled": true,
                "when": [],
                "then": [{ "kind": "setIntConstant", "class": "SampleGame", "from": 16, "to": 22 }]
            }]
        }),
    );

    let rules = project.rules().unwrap();
    let offered = rules
        .iter()
        .find(|r| r.id == "engine:widen")
        .expect("the rule should be offered");
    assert!(
        !offered.enabled,
        "a plugin may not switch its own rule on, however the file is written"
    );
    assert!(!tjlocalizer_core::rules::path(&dir.0).exists());

    assert!(project.set_rule_enabled("engine:widen", true).unwrap());
    let saved = tjlocalizer_core::rules::load(&dir.0).unwrap();
    assert_eq!(saved.len(), 1);
    assert!(saved[0].enabled);

    // And the project's copy is what now stands: the plugin cannot switch it back off.
    assert!(project.rules().unwrap()[0].enabled);
}

#[test]
fn a_plugin_cannot_name_a_format_this_build_does_not_have() {
    let dir = TempDir::new("format");
    let project =
        Project::create(&dir.0, "sample-game", &game_with_an_unrecognised_resource()).unwrap();
    write_plugin(
        &dir.0,
        "engine",
        serde_json::json!({
            "id": "engine",
            "resources": [{ "pattern": "data/lang/*.txt", "format": "unity-asset-bundle" }]
        }),
    );

    let plugins = project.plugins().unwrap();
    let problems = plugins.loaded[0].problems();
    assert!(
        problems.iter().any(|p| p.contains("unity-asset-bundle")),
        "{problems:?}"
    );
    // And it claims nothing, rather than claiming the file and reading it as something else.
    assert!(plugins.formats().is_empty());
}

/// A plugin that will not parse is reported by name and does not stop the project opening.
#[test]
fn a_broken_plugin_is_named_rather_than_swallowed() {
    let dir = TempDir::new("broken");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    std::fs::create_dir_all(dir.0.join("plugins")).unwrap();
    std::fs::write(dir.0.join("plugins/half.json"), "{ \"id\": ").unwrap();

    let plugins = project.plugins().unwrap();
    assert!(plugins.loaded.is_empty());
    assert_eq!(plugins.broken.len(), 1);
    assert!(plugins.broken[0].0.ends_with("half.json"));

    // And everything else still works.
    assert!(!project.extract().unwrap().nodes.is_empty());
}

#[test]
fn a_plugins_terms_reach_the_dictionary_without_overruling_the_projects_own() {
    let dir = TempDir::new("dictionary");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    let before = project.dictionary().unwrap().entry_count();

    write_plugin(
        &dir.0,
        "engine",
        serde_json::json!({
            "id": "engine",
            "dictionary": {
                "from": "en", "to": "vi", "name": "engine terms", "sourceNote": "the plugin",
                "entries": [
                    { "source": "Rune", "target": "Cổ Ngữ", "domain": "item", "note": "" }
                ]
            }
        }),
    );

    let dictionary = project.dictionary().unwrap();
    assert_eq!(dictionary.entry_count(), before + 1);
    let reading = dictionary
        .lookup(
            "Rune",
            &tjlocalizer_core::lang::Language::new("en"),
            &tjlocalizer_core::lang::Language::new("vi"),
            "item",
        )
        .expect("the plugin's term should be found");
    assert_eq!(reading.target, "Cổ Ngữ");
}

/// The pattern language is deliberately small, and small enough to state exhaustively.
#[test]
fn patterns_match_the_way_a_person_writing_one_expects() {
    assert!(glob("data/lang/*.txt", "data/lang/en.txt"));
    assert!(glob("data/lang/*.txt", "data/lang/vi-VN.txt"));
    assert!(!glob("data/lang/*.txt", "data/lang/en.json"));
    assert!(!glob("data/lang/*.txt", "other/lang/en.txt"));
    assert!(glob("*", "anything at all"));
    assert!(glob("*.png", "gfx/font.png"));
    assert!(glob("font?.png", "font2.png"));
    assert!(!glob("font?.png", "font22.png"));
    assert!(glob("a*b*c", "aXXbYYc"));
    assert!(!glob("a*b*c", "aXXbYY"));
    assert!(glob("", ""));
    assert!(!glob("", "x"));

    // A pattern of nothing but stars against a long name is where a naive matcher hangs.
    let long = "a".repeat(4000);
    assert!(glob("*a*a*a*a*a*b", &format!("{long}b")));
    assert!(!glob("*a*a*a*a*a*b", &long));
}

#[test]
fn no_plugins_is_not_an_error() {
    let dir = TempDir::new("none");
    let plugins = Plugins::load(&dir.0.join("plugins")).unwrap();
    assert!(plugins.is_empty());
    assert!(plugins.formats().is_empty());
    assert!(plugins.rules().is_empty());
}
