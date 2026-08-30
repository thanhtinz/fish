//! Switching a game from its glyph sheet to the handset's own font (§16).
//!
//! The other way of getting Vietnamese into a J2ME game, and usually the one people take: instead
//! of composing 134 letters into the game's sheet and teaching it that the sheet grew, stop using
//! the sheet. The body of the method that blits glyphs is replaced with a call to the platform's
//! own drawing, and every call site in the game keeps working because the signature did not move.
//!
//! This is the only bytecode this crate writes, so these tests are mostly about the shapes it
//! refuses. The proof that the bytecode is *correct* is not here - it is `tools/verify-roundtrip.sh`
//! handing the rewritten class to a real JVM's verifier.

use tjlocalizer_core::classfile::{ClassFile, MethodRef};
use tjlocalizer_core::font::device::{self, Candidate, Delegate, Job, Toolkit};

fn bitmap_font() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/BitmapFont.class"
    ))
    .expect("fixture missing - run tools/make-fixtures.sh")
}

/// The desktop stand-in for what a handset offers, so the rewrite can be exercised against types
/// an ordinary JVM has.
fn desktop_toolkit() -> Toolkit {
    let delegate = |owner: &str, name: &str, descriptor: &str| Delegate {
        called: MethodRef {
            owner: owner.to_string(),
            name: name.to_string(),
            descriptor: descriptor.to_string(),
        },
        interface: false,
    };
    Toolkit {
        draw: delegate("java/io/PrintStream", "println", "(Ljava/lang/String;)V"),
        default_font: delegate("java/lang/System", "lineSeparator", "()Ljava/lang/String;"),
        string_width: delegate("java/lang/String", "compareTo", "(Ljava/lang/String;)I"),
        char_width: delegate("java/lang/String", "indexOf", "(I)I"),
        height: delegate("java/lang/String", "length", "()I"),
    }
}

fn candidate(method: &str, descriptor: &str, job: Job) -> Candidate {
    Candidate {
        class: "BitmapFont.class".into(),
        method: method.into(),
        descriptor: descriptor.into(),
        job,
        evidence: Vec::new(),
    }
}

#[test]
fn a_drawing_method_is_rewritten_to_call_the_platforms_own() {
    let mut class = ClassFile::parse(&bitmap_font()).unwrap();

    // Before: it blits, so it holds the text it prints around the glyphs.
    assert!(class
        .string_literals()
        .iter()
        .any(|l| l.decoded.as_deref() == Some("[sheet]")));

    device::rewrite(
        &mut class,
        &candidate(
            "drawString",
            "(Ljava/io/PrintStream;Ljava/lang/String;II)V",
            Job::Draw,
        ),
        &desktop_toolkit(),
    )
    .unwrap();

    let rebuilt = ClassFile::parse(&class.write().unwrap()).unwrap();
    // The new body calls the platform.
    assert!(rebuilt
        .method_refs()
        .iter()
        .any(|r| r.owner == "java/io/PrintStream" && r.name == "println"));
    // And the old body is gone: nothing loads the sheet's own text any more.
    assert!(!rebuilt
        .string_sites()
        .unwrap()
        .iter()
        .any(|s| s.text.as_deref() == Some("[sheet]")));

    // The method is still the method: same name, same descriptor, so every call site in the game
    // still resolves.
    let method = rebuilt
        .methods()
        .unwrap()
        .into_iter()
        .find(|m| m.name == "drawString")
        .expect("the method must survive");
    assert_eq!(
        method.descriptor,
        "(Ljava/io/PrintStream;Ljava/lang/String;II)V"
    );
}

#[test]
fn the_measuring_methods_are_rewritten_too() {
    let mut class = ClassFile::parse(&bitmap_font()).unwrap();
    for (method, descriptor, job) in [
        ("stringWidth", "(Ljava/lang/String;)I", Job::StringWidth),
        ("getHeight", "()I", Job::Height),
    ] {
        device::rewrite(
            &mut class,
            &candidate(method, descriptor, job),
            &desktop_toolkit(),
        )
        .unwrap();
    }

    let rebuilt = ClassFile::parse(&class.write().unwrap()).unwrap();
    for name in ["lineSeparator", "compareTo", "length"] {
        assert!(
            rebuilt.method_refs().iter().any(|r| r.name == name),
            "{name} should be called now"
        );
    }
}

/// A method whose arguments this build cannot account for is refused rather than written badly.
/// A body that took its arguments from the wrong local slots is a class the JVM loads and a game
/// that draws nonsense.
#[test]
fn a_method_that_is_not_there_is_refused() {
    let mut class = ClassFile::parse(&bitmap_font()).unwrap();
    let refused = device::rewrite(
        &mut class,
        &candidate("drawGlyph", "(II)V", Job::Draw),
        &desktop_toolkit(),
    );
    assert!(refused.is_err());
}

/// What is recognised, and what is deliberately not.
#[test]
fn only_the_shapes_that_can_be_written_are_recognised() {
    // A surface, a string and the numbers to place it at: recognised whatever it is called.
    assert_eq!(
        device::job_of(
            "a",
            "(Ljavax/microedition/lcdui/Graphics;Ljava/lang/String;III)V"
        ),
        Some(Job::Draw)
    );
    assert_eq!(
        device::job_of(
            "renderText",
            "(Ljavax/microedition/lcdui/Graphics;Ljava/lang/String;II)V"
        ),
        Some(Job::Draw)
    );

    // The measuring shapes are ordinary - anything can take a string and return an int - so the
    // name has to agree, or half a game's methods would be offered for rewriting.
    assert_eq!(
        device::job_of("stringWidth", "(Ljava/lang/String;)I"),
        Some(Job::StringWidth)
    );
    assert_eq!(device::job_of("hash", "(Ljava/lang/String;)I"), None);
    assert_eq!(device::job_of("charWidth", "(C)I"), Some(Job::CharWidth));
    assert_eq!(device::job_of("getHeight", "()I"), Some(Job::Height));
    assert_eq!(device::job_of("getWidth", "()I"), None);

    // Shapes with nowhere to put the text.
    assert_eq!(device::job_of("draw", "(Ljava/lang/String;II)V"), None);
    assert_eq!(
        device::job_of("draw", "(Ljavax/microedition/lcdui/Graphics;II)V"),
        None
    );
    assert_eq!(device::job_of("draw", "not a descriptor"), None);
}

// -------------------------------------------------------------------------------------------
// End to end, against a project
// -------------------------------------------------------------------------------------------

use tjlocalizer_core::jar::Archive;
use tjlocalizer_core::project::Project;

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tjlocalizer-devicefont-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A game holding a class that looks like a J2ME font: it blits pieces of an image, and it has the
/// methods that go with doing so.
fn game_with_a_bitmap_font() -> Vec<u8> {
    let mut font = ClassFile::parse(&bitmap_font()).unwrap();
    // What makes a class recognisable as the one drawing the game's text: it takes a Graphics
    // apart. The fixture is compiled against desktop types, so the references are added here -
    // the detector reads the constant pool, which is where a real game's would be.
    for (name, descriptor) in [
        ("drawRegion", "(Ljavax/microedition/lcdui/Image;IIIIIIII)V"),
        ("setClip", "(IIII)V"),
    ] {
        font.add_method_ref(
            &MethodRef {
                owner: "javax/microedition/lcdui/Graphics".into(),
                name: name.into(),
                descriptor: descriptor.into(),
            },
            false,
        );
    }

    let mut archive = Archive::read(
        &std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/sample-game.jar"
        ))
        .unwrap(),
    )
    .unwrap();
    archive.insert("BitmapFont.class", font.write().unwrap());
    archive.write().unwrap()
}

#[test]
fn a_game_that_blits_its_letters_is_recognised_as_one() {
    let dir = TempDir::new("strategy");
    let project = Project::create(&dir.0, "sample-game", &game_with_a_bitmap_font()).unwrap();

    let strategy = project.font_strategy().unwrap();
    assert!(strategy.bitmap, "{strategy:?}");
    assert!(strategy.worth_switching());
    assert!(
        strategy
            .evidence
            .iter()
            .any(|e| e.contains("BitmapFont.class")),
        "{strategy:?}"
    );

    let candidates = project.system_font_candidates().unwrap();
    let drawing = candidates
        .iter()
        .find(|c| c.job == Job::Draw)
        .expect("the drawing method should be offered");
    assert_eq!(drawing.class, "BitmapFont.class");
    assert_eq!(drawing.method, "drawString");
    assert!(
        !drawing.evidence.is_empty(),
        "a candidate carries its reasons"
    );
    // The measuring methods come with it: text drawn by the handset and measured from the old
    // sheet is text in the wrong places.
    assert!(candidates.iter().any(|c| c.job == Job::StringWidth));
    assert!(candidates.iter().any(|c| c.job == Job::Height));
}

/// The switch is offered as a rule, arrives off, and changes what the build is judged against only
/// once somebody turns it on.
#[test]
fn switching_is_a_rule_that_starts_off() {
    let dir = TempDir::new("rule");
    let project = Project::create(&dir.0, "sample-game", &game_with_a_bitmap_font()).unwrap();

    let written = project.write_system_font_rules().unwrap();
    assert_eq!(written.len(), 1, "one rule per class");
    assert!(!written[0].enabled);
    assert!(written[0].id.starts_with("system-font-"));
    assert!(!project.switched_to_device_font().unwrap());

    // Off, so the game is still drawing from whatever sheet it has - which this project has not
    // declared, so there is still no coverage to report.
    assert!(project.font_coverage().unwrap().is_none());

    let plan = project
        .plan_rules()
        .unwrap()
        .into_iter()
        .find(|p| p.id == written[0].id)
        .unwrap();
    assert!(!plan.effects.is_empty(), "it says what it would do");
    assert!(
        plan.effects[0].contains("handset's own font"),
        "{:?}",
        plan.effects
    );

    assert!(project.set_rule_enabled(&written[0].id, true).unwrap());
    assert!(project.switched_to_device_font().unwrap());

    // Now the game will draw with the handset's font, so every Vietnamese letter is covered -
    // which is the whole point of the switch.
    let coverage = project
        .font_coverage()
        .unwrap()
        .expect("coverage now known");
    assert!(coverage.missing_for_vietnamese().is_empty());
}

/// A game with no such class is told so, rather than given a rule that would do nothing.
#[test]
fn a_game_with_no_font_class_gets_no_rule() {
    let dir = TempDir::new("none");
    let plain = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/sample-game.jar"
    ))
    .unwrap();
    let project = Project::create(&dir.0, "sample-game", &plain).unwrap();

    assert!(project.system_font_candidates().unwrap().is_empty());
    let refused = project.write_system_font_rules().unwrap_err().to_string();
    assert!(
        refused.contains("nothing in this game looks like a font class"),
        "{refused}"
    );
    assert!(!project.font_strategy().unwrap().worth_switching());
}

/// A rule written against one version of a class must not write a method body into another.
#[test]
fn a_rule_for_a_class_that_changed_refuses_to_run() {
    let dir = TempDir::new("moved");
    let project = Project::create(&dir.0, "sample-game", &game_with_a_bitmap_font()).unwrap();
    let written = project.write_system_font_rules().unwrap();
    project.set_rule_enabled(&written[0].id, true).unwrap();
    assert!(project.switched_to_device_font().unwrap());

    // The game is updated: the class is not the one measured.
    let mut archive = Archive::read(&game_with_a_bitmap_font()).unwrap();
    let mut font = ClassFile::parse(&archive.get("BitmapFont.class").unwrap().data).unwrap();
    font.add_string("a later version of this class").unwrap();
    archive.insert("BitmapFont.class", font.write().unwrap());

    let moved = TempDir::new("moved-2");
    let updated = Project::create(&moved.0, "sample-game", &archive.write().unwrap()).unwrap();
    updated.put_rule(written[0].clone()).unwrap();
    updated.set_rule_enabled(&written[0].id, true).unwrap();

    assert!(
        !updated.switched_to_device_font().unwrap(),
        "a rule that does not fit must not be counted as having run"
    );
    let plan = updated
        .plan_rules()
        .unwrap()
        .into_iter()
        .find(|p| p.id == written[0].id)
        .unwrap();
    assert!(!plan.ready());
    assert!(!plan.unmet.is_empty());
}
