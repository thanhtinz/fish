//! Finding a J2ME emulator that is already on this machine.
//!
//! Nothing here downloads anything, and neither does the code it tests. What matters most is the
//! *empty* answer: a search that finds nothing has to say where it looked, because "no emulator
//! found" is not something anybody can act on and a list of the places checked is.

use tjlocalizer_core::emulator;

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("tjlocalizer-emu-{tag}-{unique}"));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An emulator jar in one of the places people keep them is found, and the command built for it
/// puts a JVM in front - which is how every jar-based emulator actually runs.
#[test]
fn an_emulator_jar_in_a_home_directory_is_found() {
    let home = TempDir::new("found");
    std::fs::create_dir_all(home.0.join("Downloads")).unwrap();
    std::fs::write(home.0.join("Downloads/freej2me.jar"), b"not really a jar").unwrap();

    let found = emulator::find(Some(&home.0));
    // Every machine with a JVM finds this; one without has no way to run a jar and says nothing,
    // which is the right answer rather than a broken test.
    if !emulator::java_available() {
        assert!(found.is_empty());
        return;
    }

    let one = found
        .iter()
        .find(|f| f.path.ends_with("freej2me.jar"))
        .expect("the jar was not found");
    assert_eq!(one.name, "FreeJ2ME");
    assert!(one.emulator.command.contains("java"), "{:?}", one.emulator);
    assert!(
        one.emulator.args.iter().any(|a| a == "-jar"),
        "{:?}",
        one.emulator
    );
    // `{game}` is what `Emulator::arguments` fills in, and without it the path is appended in the
    // wrong place - after the emulator's own jar rather than as its argument.
    assert!(
        one.emulator.args.iter().any(|a| a == "{game}"),
        "{:?}",
        one.emulator
    );
    assert!(one.evidence.contains("freej2me.jar"), "{}", one.evidence);
}

/// A file that is not one of the names an emulator goes by is not one, however hopeful it looks.
#[test]
fn an_unrelated_jar_is_not_mistaken_for_an_emulator() {
    let home = TempDir::new("unrelated");
    std::fs::create_dir_all(home.0.join("Downloads")).unwrap();
    std::fs::write(home.0.join("Downloads/some-game.jar"), b"x").unwrap();
    std::fs::write(home.0.join("Downloads/emulator-notes.txt"), b"x").unwrap();

    let found = emulator::find(Some(&home.0));
    assert!(
        !found.iter().any(|f| f.path.starts_with(&home.0)),
        "{found:?}"
    );
}

/// The empty result is the one that has to be useful, so the places that were searched are
/// reported rather than left for somebody to guess at.
#[test]
fn the_search_can_say_where_it_looked() {
    let home = TempDir::new("where");
    let places = emulator::searched(Some(&home.0));

    assert!(!places.is_empty());
    assert!(
        places.iter().any(|p| p.starts_with(&home.0)),
        "the home directory was not among the places searched"
    );
    assert!(
        places.iter().any(|p| p.ends_with("Downloads")),
        "Downloads was not searched, and it is where people put these"
    );
}

/// Called with no home directory it must still work rather than panic: a machine where HOME is
/// unset is unusual, not impossible, and PATH is still worth searching.
#[test]
fn a_machine_with_no_home_directory_still_gets_an_answer() {
    let _ = emulator::find(None);
    assert!(!emulator::searched(None).is_empty());
}
