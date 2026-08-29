//! Per-game patches (§19).
//!
//! The whole value of this module is what it refuses to do. A rule that patched whatever it found
//! would be worse than no rule engine at all: it would corrupt games quietly, one constant at a
//! time, and the corruption would show up as a rendering bug in a build nobody could explain.
//! So most of these tests are about a rule declining to run.

use tjlocalizer_core::classfile::ClassFile;
use tjlocalizer_core::jar::{sha256_hex, Archive};
use tjlocalizer_core::project::Project;
use tjlocalizer_core::rules::{self, Action, Condition, Rule};

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
        let path = std::env::temp_dir().join(format!("tjlocalizer-rules-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn enabled(mut rule: Rule) -> Rule {
    rule.enabled = true;
    rule
}

fn a_string_in_the_game() -> String {
    let archive = Archive::read(&fixture()).unwrap();
    let class = archive.get("SampleGame.class").unwrap();
    ClassFile::parse(&class.data)
        .unwrap()
        .string_literals()
        .into_iter()
        .find_map(|l| l.decoded)
        .expect("the fixture has no string literals")
}

#[test]
fn a_rule_written_for_another_version_of_the_game_refuses_to_run() {
    let dir = TempDir::new("mismatch");
    let mut archive = Archive::read(&fixture()).unwrap();

    let mut rule = enabled(Rule::new("swap", "replace the artwork"));
    rule.when = vec![Condition::EntrySha256 {
        entry: "levels.properties".into(),
        sha256: "0".repeat(64),
    }];
    rule.then = vec![Action::ReplaceEntry {
        entry: "levels.properties".into(),
        from: "replacement.bin".into(),
    }];
    std::fs::write(dir.0.join("replacement.bin"), b"new").unwrap();

    let before = archive.get("levels.properties").unwrap().data.clone();
    let applied = rules::apply(&[rule.clone()], &mut archive, &dir.0).unwrap();

    assert!(applied.rules.is_empty(), "the rule should not have run");
    assert_eq!(archive.get("levels.properties").unwrap().data, before);

    // And it says why, rather than reporting nothing to do.
    let plan = &rules::plan(&[rule], &archive, &dir.0).unwrap()[0];
    assert_eq!(plan.unmet.len(), 1);
    assert!(
        plan.unmet[0].contains("not the file this rule was written against"),
        "unhelpful: {}",
        plan.unmet[0]
    );
    assert!(!plan.ready());
}

#[test]
fn a_matching_rule_replaces_the_entry_and_says_it_did() {
    let dir = TempDir::new("replace");
    let mut archive = Archive::read(&fixture()).unwrap();
    let sha = sha256_hex(&archive.get("levels.properties").unwrap().data);
    std::fs::write(dir.0.join("replacement.bin"), b"new contents").unwrap();

    let mut rule = enabled(Rule::new("swap", "replace the artwork"));
    rule.when = vec![Condition::EntrySha256 {
        entry: "levels.properties".into(),
        sha256: sha,
    }];
    rule.then = vec![Action::ReplaceEntry {
        entry: "levels.properties".into(),
        from: "replacement.bin".into(),
    }];

    let plan = &rules::plan(&[rule.clone()], &archive, &dir.0).unwrap()[0];
    assert!(plan.ready(), "unmet: {:?}", plan.unmet);
    assert_eq!(plan.effects.len(), 1);

    let applied = rules::apply(&[rule], &mut archive, &dir.0).unwrap();
    assert_eq!(applied.rules, vec!["swap"]);
    assert_eq!(applied.entries_replaced, 1);
    assert_eq!(
        archive.get("levels.properties").unwrap().data,
        b"new contents"
    );
}

/// A rule is a change to how somebody's game behaves. Nothing runs because it was written.
#[test]
fn a_rule_that_was_never_switched_on_does_nothing() {
    let dir = TempDir::new("off");
    let mut archive = Archive::read(&fixture()).unwrap();
    std::fs::write(dir.0.join("replacement.bin"), b"new").unwrap();

    let mut rule = Rule::new("swap", "replace the artwork");
    rule.then = vec![Action::ReplaceEntry {
        entry: "levels.properties".into(),
        from: "replacement.bin".into(),
    }];

    let applied = rules::apply(&[rule.clone()], &mut archive, &dir.0).unwrap();
    assert!(applied.rules.is_empty());

    // The plan still shows what it would do, because that is how somebody decides to enable it.
    let plan = &rules::plan(&[rule], &archive, &dir.0).unwrap()[0];
    assert!(plan.unmet.is_empty());
    assert_eq!(plan.effects.len(), 1);
    assert!(!plan.ready(), "an unenabled rule is not ready to run");
}

#[test]
fn a_constant_is_changed_only_in_the_class_the_rule_named() {
    let dir = TempDir::new("constant");
    let mut archive = Archive::read(&fixture()).unwrap();
    let text = a_string_in_the_game();

    let mut rule = enabled(Rule::new("relabel", "change one literal"));
    rule.when = vec![Condition::StringConstant {
        class: "SampleGame.class".into(),
        text: text.clone(),
    }];
    rule.then = vec![Action::SetStringConstant {
        class: "SampleGame.class".into(),
        from: text.clone(),
        to: "đã thay".into(),
    }];

    let applied = rules::apply(&[rule], &mut archive, &dir.0).unwrap();
    assert_eq!(applied.constants_changed, 1);

    let patched = ClassFile::parse(&archive.get("SampleGame.class").unwrap().data).unwrap();
    let literals: Vec<String> = patched
        .string_literals()
        .into_iter()
        .filter_map(|l| l.decoded)
        .collect();
    assert!(literals.iter().any(|l| l == "đã thay"));
    assert!(
        !literals.contains(&text),
        "the original literal is still there"
    );
}

/// A rule naming a class the game does not have is a rule for a different game.
#[test]
fn a_rule_for_a_class_that_is_not_there_reports_it_rather_than_failing() {
    let dir = TempDir::new("missing-class");
    let mut archive = Archive::read(&fixture()).unwrap();

    let mut rule = enabled(Rule::new("elsewhere", "patch another game"));
    rule.when = vec![Condition::IntConstant {
        class: "OtherGame.class".into(),
        value: 16,
    }];
    rule.then = vec![Action::SetIntConstant {
        class: "OtherGame.class".into(),
        from: 16,
        to: 22,
    }];

    let plan = &rules::plan(&[rule.clone()], &archive, &dir.0).unwrap()[0];
    assert_eq!(plan.unmet, vec!["the game has no OtherGame.class"]);
    assert!(plan.effects.is_empty());

    rules::apply(&[rule], &mut archive, &dir.0).unwrap();
}

/// Numbers are read from the archive, not repeated from the rule.
#[test]
fn the_plan_counts_what_is_actually_there() {
    let dir = TempDir::new("counts");
    let archive = Archive::read(&fixture()).unwrap();

    let mut rule = enabled(Rule::new("nothing", "change a number that is not there"));
    rule.then = vec![Action::SetIntConstant {
        class: "SampleGame.class".into(),
        from: -424_242,
        to: 1,
    }];

    let plan = &rules::plan(&[rule], &archive, &dir.0).unwrap()[0];
    assert!(
        plan.effects.is_empty(),
        "a rule matching nothing claimed an effect: {:?}",
        plan.effects
    );
    assert!(
        !plan.ready(),
        "a rule that would change nothing is not ready"
    );
}

#[test]
fn rules_survive_being_written_and_read_back() {
    let dir = TempDir::new("roundtrip");
    let mut rule = enabled(Rule::new("install-font", "put the composed sheet in"));
    rule.when = vec![Condition::ProjectFile {
        path: "fonts/extended.png".into(),
    }];
    rule.then = vec![Action::ReplaceEntry {
        entry: "/font.png".into(),
        from: "fonts/extended.png".into(),
    }];

    rules::save(&dir.0, std::slice::from_ref(&rule)).unwrap();
    let read = rules::load(&dir.0).unwrap();

    assert_eq!(read.len(), 1);
    assert_eq!(read[0].id, "install-font");
    assert!(read[0].enabled);
    assert_eq!(read[0].when, rule.when);
    assert_eq!(read[0].then, rule.then);
}

#[test]
fn a_project_with_no_rules_file_has_no_rules_rather_than_an_error() {
    let dir = TempDir::new("empty");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    assert!(project.rules().unwrap().is_empty());
    assert!(project.plan_rules().unwrap().is_empty());
}

/// The build has to record what was done to the game, or a shipped patch is untraceable.
#[test]
fn a_build_records_the_rules_that_ran() {
    let dir = TempDir::new("build");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    project.extract().unwrap();

    std::fs::write(dir.0.join("replacement.bin"), b"patched").unwrap();
    let mut rule = enabled(Rule::new("swap", "replace a resource"));
    rule.when = vec![Condition::EntryExists {
        entry: "levels.properties".into(),
    }];
    rule.then = vec![Action::ReplaceEntry {
        entry: "levels.properties".into(),
        from: "replacement.bin".into(),
    }];
    project.put_rule(rule).unwrap();

    let language = project.active_targets()[0].language.clone();
    let record = project.build(&language).unwrap();
    assert_eq!(record.rules.rules, vec!["swap"]);
    assert_eq!(record.rules.entries_replaced, 1);

    // And the patch is in the file that ships, not only in the record.
    let output = project.output_path(&language).unwrap().unwrap();
    let built = Archive::read(&std::fs::read(output).unwrap()).unwrap();
    assert_eq!(built.get("levels.properties").unwrap().data, b"patched");
}

#[test]
fn switching_a_rule_off_leaves_it_in_the_project() {
    let dir = TempDir::new("toggle");
    let project = Project::create(&dir.0, "sample-game", &fixture()).unwrap();
    project
        .put_rule(enabled(Rule::new("swap", "a rule")))
        .unwrap();

    assert!(project.set_rule_enabled("swap", false).unwrap());
    assert_eq!(project.rules().unwrap().len(), 1);
    assert!(!project.rules().unwrap()[0].enabled);

    assert!(
        !project.set_rule_enabled("nonexistent", true).unwrap(),
        "toggling a rule that is not there should say so"
    );
    assert!(project.remove_rule("swap").unwrap());
    assert!(project.rules().unwrap().is_empty());
}

/// Composing a sheet and actually shipping it, which until now stopped one step short.
///
/// The font engine could produce the artwork but not put it in the game, so a build with perfect
/// Vietnamese still failed its own glyph check. This is that gap closed: the rule installs the
/// sheet, and because it does, the coverage the build is judged against is the sheet that ships.
#[test]
fn installing_the_composed_sheet_is_what_makes_the_build_pass_its_glyph_check() {
    use tjlocalizer_core::font::sheet::{Grid, Image};
    use tjlocalizer_core::project::FontProfile;

    let cell = 12u32;
    let columns = 16u32;
    let characters: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    let rows = (characters.len() as u32).div_ceil(columns);
    let grid = Grid {
        cell_width: cell,
        cell_height: cell,
        columns,
        rows,
    };
    let mut image = Image::new(columns * cell, rows * cell);
    for (i, c) in characters.iter().enumerate() {
        if *c == ' ' {
            continue;
        }
        let (ox, oy) = grid.cell_origin(i as u32);
        let seed = (*c as u32).wrapping_mul(2_654_435_761);
        for y in 0..4u32 {
            for x in 0..6u32 {
                let edge = x == 0 || x == 5 || y == 0 || y == 3;
                let inked = edge || (seed >> ((y - 1) * 4 + (x - 1).min(3))) & 1 == 1;
                if inked {
                    image.set(ox + 3 + x, oy + 6 + y, [220, 200, 120, 255]);
                }
            }
        }
    }
    let mut source = Archive::read(&fixture()).unwrap();
    source.insert("font.png", image.encode_png().unwrap());

    let dir = TempDir::new("install-font");
    let mut project = Project::create(&dir.0, "sample-game", &source.write().unwrap()).unwrap();
    project.extract().unwrap();
    project.profile_mut().font = Some(FontProfile {
        entry: "font.png".into(),
        grid: Some(grid),
        order: String::new(),
        device_font: false,
        mark_library: None,
        marks_from: None,
    });
    project.save().unwrap();

    // Before composing there is nothing to install, and the rule says so rather than writing a
    // rule that would replace the sheet with a file that does not exist.
    assert!(project.font_install_rule().is_err());

    project.compose_font(None).unwrap().unwrap();
    let before = project.font_coverage().unwrap().unwrap();
    assert_eq!(
        before.missing_for_vietnamese().len(),
        134,
        "the game's own sheet should still cover no Vietnamese"
    );

    let rule = project.font_install_rule().unwrap();
    project.put_rule(rule).unwrap();

    // Written, but off. Nothing about generating a rule runs it.
    assert_eq!(
        project
            .font_coverage()
            .unwrap()
            .unwrap()
            .missing_for_vietnamese()
            .len(),
        134
    );

    project.set_rule_enabled("install-font", true).unwrap();
    let after = project.font_coverage().unwrap().unwrap();
    assert!(
        after.missing_for_vietnamese().is_empty(),
        "still missing {:?}",
        after.missing_for_vietnamese()
    );

    let language = project.active_targets()[0].language.clone();
    let record = project.build(&language).unwrap();
    assert_eq!(record.rules.rules, vec!["install-font"]);

    let output = project.output_path(&language).unwrap().unwrap();
    let built = Archive::read(&std::fs::read(output).unwrap()).unwrap();
    let shipped = &built.get("font.png").unwrap().data;
    assert_eq!(
        shipped,
        &std::fs::read(dir.0.join("fonts/extended.png")).unwrap(),
        "the game shipped its original sheet"
    );

    // The composed sheet is taller than the one it replaces - which is the part a person still
    // has to teach the game about, and the reason the rule's description says so.
    let original_height =
        tjlocalizer_core::font::sheet::Image::decode_png(&source.get("font.png").unwrap().data)
            .unwrap()
            .height;
    let shipped_height = tjlocalizer_core::font::sheet::Image::decode_png(shipped)
        .unwrap()
        .height;
    assert!(shipped_height > original_height);
}
