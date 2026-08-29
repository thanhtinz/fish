//! Recognising what kind of game is in front of us (§7).
//!
//! The project began with J2ME JARs, and the rest of the world does not ship those. What matters
//! here as much as recognising a package is being honest about it: an Android package can be read
//! and cannot be rebuilt into something a device will install, and a tool that quietly produced
//! one anyway would waste somebody's afternoon.

use tjlocalizer_core::jar::Archive;
use tjlocalizer_core::package::{self, Kind};
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
        let path = std::env::temp_dir().join(format!("tjlocalizer-package-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An archive shaped like an Android package: a manifest, some bytecode, and its strings.
fn apk() -> Vec<u8> {
    let mut archive = Archive::read(&fixture()).unwrap();
    archive.remove("SampleGame.class");
    archive.remove("levels.properties");
    archive.insert("AndroidManifest.xml", b"binary xml, not text".to_vec());
    archive.insert("classes.dex", b"dex\n035\0padding".to_vec());
    archive.insert("resources.arsc", vec![2, 0, 12, 0]);
    archive.insert(
        "res/values/strings.xml",
        b"<resources>\n    <string name=\"start\">Start Game</string>\n</resources>\n".to_vec(),
    );
    archive.write().unwrap()
}

fn ipa() -> Vec<u8> {
    let mut archive = Archive::read(&fixture()).unwrap();
    archive.remove("SampleGame.class");
    archive.insert("Payload/Fishing.app/Info.plist", b"<plist/>".to_vec());
    archive.insert(
        "Payload/Fishing.app/en.lproj/Localizable.strings",
        b"\"start\" = \"Start Game\";\n".to_vec(),
    );
    archive.write().unwrap()
}

#[test]
fn a_midlet_is_still_recognised_as_one() {
    let found = package::detect(&Archive::read(&fixture()).unwrap());
    assert_eq!(found.kind, Kind::Midlet);
    assert!(found.kind.can_repackage());
    assert!(found.kind.repackaging_note().is_none());
    assert!(
        found.evidence.iter().any(|e| e.contains("MIDlet-1")),
        "{:?}",
        found.evidence
    );
}

#[test]
fn an_android_package_is_recognised_and_its_strings_are_readable() {
    let found = package::detect(&Archive::read(&apk()).unwrap());
    assert_eq!(found.kind, Kind::Apk);

    let strings = found
        .readable
        .iter()
        .find(|r| r.entry == "res/values/strings.xml")
        .expect("the string table was not readable");
    assert_eq!(strings.format, "android-strings");
    assert_eq!(strings.fields, 1);
}

/// The part that matters more than the recognition: what cannot be done, said up front.
#[test]
fn an_android_package_says_it_cannot_be_signed() {
    let found = package::detect(&Archive::read(&apk()).unwrap());
    assert!(!found.kind.can_repackage());
    let note = found.kind.repackaging_note().unwrap();
    assert!(
        note.contains("signature") || note.contains("sign"),
        "{note}"
    );
}

/// Text this build cannot open is listed rather than ignored: a translator who cannot see that a
/// game keeps half its dialogue somewhere unreadable will think the game is half translated.
#[test]
fn unreadable_text_is_named_rather_than_passed_over() {
    let found = package::detect(&Archive::read(&apk()).unwrap());
    let named: Vec<&str> = found.opaque.iter().map(|o| o.entry.as_str()).collect();

    assert!(named.contains(&"classes.dex"), "{named:?}");
    assert!(named.contains(&"resources.arsc"), "{named:?}");
    for opaque in &found.opaque {
        assert!(!opaque.reason.is_empty(), "{} has no reason", opaque.entry);
    }
}

#[test]
fn an_ios_archive_is_recognised_by_its_payload() {
    let found = package::detect(&Archive::read(&ipa()).unwrap());
    assert_eq!(found.kind, Kind::Ipa);
    assert!(!found.kind.can_repackage());
    assert!(found
        .readable
        .iter()
        .any(|r| r.format == "apple-strings" && r.fields == 1));
}

/// Importing one has to work end to end, or none of the above is reachable.
#[test]
fn an_android_package_can_be_imported_and_its_text_extracted() {
    let dir = TempDir::new("import-apk");
    let project = Project::create(&dir.0, "fishing", &apk()).unwrap();

    // Stored under its own extension: an APK kept as a .jar is still an APK, and nothing that
    // opens it knows that - starting with the person looking for it in a file manager.
    assert!(
        dir.0.join("original/fishing.apk").is_file(),
        "{:?}",
        std::fs::read_dir(dir.0.join("original"))
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name()))
            .collect::<Vec<_>>()
    );

    let graph = project.extract().unwrap();
    let start = graph
        .nodes
        .iter()
        .find(|n| n.source_text == "Start Game")
        .expect("the Android string was not extracted");
    assert!(start.context.is_translatable());

    // And the build says what it still needs from a person, rather than handing over a file that
    // will not install and letting them find out.
    let language = project.active_targets()[0].language.clone();
    let mut translations = project.translations(&language).unwrap();
    translations.set(start.id.clone(), "Bắt đầu");
    project.save_translations(&language, &translations).unwrap();

    let record = project.build(&language).unwrap();
    let note = record
        .validation
        .findings
        .iter()
        .find(|f| f.check == "package.signature")
        .expect("nothing said the package would need re-signing");
    assert!(note.detail.contains("Android"), "{}", note.detail);

    // And the checks that are rules about JAR files stood down: an Android package has no MIDlet
    // entry point, and reporting that as an error would be reporting that it is not a J2ME game.
    let jar_rules: Vec<&str> = record
        .validation
        .findings
        .iter()
        .filter(|f| f.check == "entry_point" || f.check == "font")
        .map(|f| f.check.as_str())
        .collect();
    assert!(
        jar_rules.is_empty(),
        "JAR rules ran against an APK: {jar_rules:?}"
    );
    assert!(
        record.validation.is_ok(),
        "{:?}",
        record.validation.findings
    );

    // The output keeps the kind it came in as.
    let output = project.output_path(&language).unwrap().unwrap();
    assert_eq!(output.extension().unwrap(), "apk");

    // And the translation really is in it, in the format it came from.
    let built = Archive::read(&std::fs::read(output).unwrap()).unwrap();
    let strings =
        String::from_utf8(built.get("res/values/strings.xml").unwrap().data.clone()).unwrap();
    assert!(strings.contains(">Bắt đầu<"), "{strings}");
    assert!(
        strings.contains("<resources>"),
        "the file was rewritten: {strings}"
    );
}

/// A package's own structure is never game text, on any platform.
///
/// The J2ME version of this was found the hard way: `MIDlet-1: Sample Game,/icon.png,SampleGame`
/// was offered to a translator, and translating it renames the entry point of a game that then
/// installs and refuses to start. An Android manifest and an iOS Info.plist are the same file in
/// a different costume - the package name, the permissions, the class of every screen.
#[test]
fn a_platform_manifest_is_never_offered_as_game_text() {
    let mut archive = Archive::read(&apk()).unwrap();
    // Written as plain text, which is the case a "does this decode?" test would happily accept.
    archive.insert(
        "AndroidManifest.xml",
        b"<manifest package=\"com.example.fishing\">\n  <string>Fishing</string>\n</manifest>\n"
            .to_vec(),
    );
    archive.insert(
        "Payload/App.app/Info.plist",
        b"<string>Fishing</string>".to_vec(),
    );

    let dir = TempDir::new("manifest");
    let project = Project::create(&dir.0, "fishing", &archive.write().unwrap()).unwrap();
    let graph = project.extract().unwrap();

    for node in &graph.nodes {
        let where_from = format!("{:?}", node.source);
        assert!(
            !where_from.contains("AndroidManifest") && !where_from.contains("Info.plist"),
            "{} was offered from {where_from}",
            node.source_text
        );
    }
}
