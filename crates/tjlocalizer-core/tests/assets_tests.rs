//! Words painted into artwork (§17).
//!
//! The failure this guards against is the quietest one in the project: a build that passes every
//! check, reports every string translated, and shows the player an English button, because the
//! word was never a string at all.

use tjlocalizer_core::assets::{self, TextAsset};
use tjlocalizer_core::font::sheet::Image;
use tjlocalizer_core::jar::Archive;
use tjlocalizer_core::project::Project;
use tjlocalizer_core::validate::Severity;

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
        let path = std::env::temp_dir().join(format!("tjlocalizer-assets-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A wide, short, two-colour image with one band of ink: a button label.
fn label(width: u32, height: u32) -> Vec<u8> {
    let mut image = Image::new(width, height);
    for y in (height / 3)..(height * 2 / 3) {
        for x in 4..(width - 4) {
            if (x / 3) % 2 == 0 {
                image.set(x, y, [240, 240, 240, 255]);
            }
        }
    }
    image.encode_png().unwrap()
}

/// Many colours, ink everywhere, no clear bands: a background.
fn scenery(width: u32, height: u32) -> Vec<u8> {
    let mut image = Image::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let v = |m: u32, n: u32| ((x * m + y * n) % 256) as u8;
            image.set(x, y, [v(7, 3), v(3, 11), v(13, 5), 255]);
        }
    }
    image.encode_png().unwrap()
}

fn game_with_images() -> Vec<u8> {
    let mut archive = Archive::read(&fixture()).unwrap();
    archive.insert("sky.png", scenery(96, 96));
    archive.insert("start_btn.png", label(96, 24));
    archive.write().unwrap()
}

#[test]
fn a_button_label_is_flagged_and_a_background_is_not() {
    let archive = Archive::read(&game_with_images()).unwrap();
    let assets = assets::scan(&archive).unwrap();

    let button = assets.iter().find(|a| a.entry == "start_btn.png").unwrap();
    let sky = assets.iter().find(|a| a.entry == "sky.png").unwrap();

    assert!(button.worth_checking(), "the button was not flagged");
    assert!(
        !sky.worth_checking(),
        "scenery was flagged as a label: {:?}",
        sky.hints
    );
    // Evidence rather than a verdict: each hint is something a person can check by looking.
    assert!(
        button
            .hints
            .iter()
            .any(|h| matches!(h, assets::Hint::ShapeOfALine { .. })),
        "{:?}",
        button.hints
    );
    // Flagged ones come first, since that is the list somebody is going to read.
    assert_eq!(assets[0].entry, "start_btn.png");
}

/// Marking an entry the game does not have is a typo, and saying so beats recording a promise
/// about a file that will never be checked.
#[test]
fn an_entry_the_game_does_not_have_cannot_be_marked() {
    let dir = TempDir::new("unknown");
    let mut project = Project::create(&dir.0, "sample-game", &game_with_images()).unwrap();

    let result = project.mark_text_asset(TextAsset {
        entry: "nope.png".into(),
        says: String::new(),
        replacement: None,
    });
    assert!(result.is_err());
    assert!(project.profile().text_assets.is_empty());
}

/// The whole point: a build that is otherwise perfect must still say the artwork is untranslated.
#[test]
fn a_marked_image_with_no_replacement_is_reported_in_the_build() {
    let dir = TempDir::new("unreplaced");
    let mut project = Project::create(&dir.0, "sample-game", &game_with_images()).unwrap();
    project.extract().unwrap();
    project
        .mark_text_asset(TextAsset {
            entry: "start_btn.png".into(),
            says: "START".into(),
            replacement: None,
        })
        .unwrap();

    let language = project.active_targets()[0].language.clone();
    let record = project.build(&language).unwrap();

    let finding = record
        .validation
        .findings
        .iter()
        .find(|f| f.check == "asset.text")
        .expect("the untranslated artwork was not reported");
    assert_eq!(
        finding.severity,
        Severity::Warning,
        "shipping it is a decision, not a defect"
    );
    assert!(finding.detail.contains("START"), "{}", finding.detail);
    assert!(
        record.validation.is_ok(),
        "a warning must not fail the build"
    );
}

/// Having the redrawn file is not the same as shipping it: installing an image is a rule, and a
/// rule that was written but never switched on leaves the artwork untouched.
#[test]
fn a_redrawn_image_that_never_reached_the_build_is_reported() {
    let dir = TempDir::new("uninstalled");
    let mut project = Project::create(&dir.0, "sample-game", &game_with_images()).unwrap();
    project.extract().unwrap();

    // A different picture from the original, or "the build still carries the original" would be
    // true and false at once.
    std::fs::create_dir_all(dir.0.join("assets")).unwrap();
    std::fs::write(dir.0.join("assets/start_btn.png"), label(96, 28)).unwrap();
    project
        .mark_text_asset(TextAsset {
            entry: "start_btn.png".into(),
            says: "START".into(),
            replacement: Some("assets/start_btn.png".into()),
        })
        .unwrap();

    let language = project.active_targets()[0].language.clone();
    let record = project.build(&language).unwrap();
    let detail = record
        .validation
        .findings
        .iter()
        .find(|f| f.check == "asset.text")
        .map(|f| f.detail.clone())
        .expect("nothing said the redrawn image had not been installed");
    assert!(detail.contains("a rule has to install it"), "{detail}");
}

/// And once a rule installs it, the build stops complaining - otherwise the warning would be
/// noise a person learns to ignore.
#[test]
fn installing_the_redrawn_image_clears_the_warning() {
    use tjlocalizer_core::rules::{Action, Condition, Rule};

    let dir = TempDir::new("installed");
    let mut project = Project::create(&dir.0, "sample-game", &game_with_images()).unwrap();
    project.extract().unwrap();

    // A different picture from the original, or the test would pass without installing anything.
    std::fs::create_dir_all(dir.0.join("assets")).unwrap();
    std::fs::write(dir.0.join("assets/start_btn.png"), label(96, 28)).unwrap();
    project
        .mark_text_asset(TextAsset {
            entry: "start_btn.png".into(),
            says: "START".into(),
            replacement: Some("assets/start_btn.png".into()),
        })
        .unwrap();

    let mut rule = Rule::new("redraw-start", "install the redrawn button");
    rule.enabled = true;
    rule.when = vec![Condition::EntryExists {
        entry: "start_btn.png".into(),
    }];
    rule.then = vec![Action::ReplaceEntry {
        entry: "start_btn.png".into(),
        from: "assets/start_btn.png".into(),
    }];
    project.put_rule(rule).unwrap();

    let language = project.active_targets()[0].language.clone();
    let record = project.build(&language).unwrap();
    let asset_findings: Vec<&str> = record
        .validation
        .findings
        .iter()
        .filter(|f| f.check == "asset.text")
        .map(|f| f.detail.as_str())
        .collect();
    assert!(
        asset_findings.is_empty(),
        "still complaining after the image was installed: {asset_findings:?}"
    );
}

/// The hints cross into an interface as JSON, and a field that arrives under the wrong name reads
/// as missing there rather than as an error. This one did: the enum renamed its variants and left
/// their fields alone, so a percentage showed up on screen as "undefined".
#[test]
fn a_hint_carries_its_numbers_under_the_names_an_interface_expects() {
    let archive = Archive::read(&game_with_images()).unwrap();
    let button = assets::scan(&archive)
        .unwrap()
        .into_iter()
        .find(|a| a.entry == "start_btn.png")
        .unwrap();

    let json = serde_json::to_string(&button.hints).unwrap();
    for wanted in ["\"kind\"", "inkPercent", "shapeOfALine"] {
        assert!(json.contains(wanted), "{wanted} is missing from {json}");
    }
    assert!(
        !json.contains("ink_percent"),
        "a field crossed over under its Rust name: {json}"
    );
}
