//! Finding a J2ME emulator that is already on this machine (§25).
//!
//! `Project::play` runs a command its owner recorded. That is the right shape - nothing read out
//! of a game can influence what gets executed - but it puts the whole burden of "which emulator,
//! where is it, what arguments" on somebody who wanted to see their translation on a screen.
//!
//! So this looks. It **only looks**: nothing here downloads an emulator, installs one, or suggests
//! a place to get one from. What it finds is offered with the path it was found at, and recording
//! it is still a separate act by a person.
//!
//! The important case is the empty one. A search that finds nothing has to say **where it looked**,
//! because "no emulator found" is not something anybody can act on, and a list of the eight places
//! that were checked is.

use crate::regress::Emulator;
use std::path::{Path, PathBuf};

/// An emulator this machine appears to have.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// What it is, as a person would name it.
    pub name: &'static str,
    /// Where it was found.
    pub path: PathBuf,
    /// The command that would run it.
    pub emulator: Emulator,
    /// How it was found, so a wrong answer can be argued with.
    pub evidence: String,
}

/// The names each emulator is known by on disk.
///
/// Two kinds, and they run differently: a program that can be executed directly, and a `.jar` that
/// needs a JVM in front of it. Both are common - most J2ME emulators still ship as a jar.
struct Known {
    name: &'static str,
    /// Executable names, looked for on PATH.
    programs: &'static [&'static str],
    /// Jar file names, looked for in the places people keep them.
    jars: &'static [&'static str],
}

const KNOWN: &[Known] = &[
    Known {
        name: "FreeJ2ME",
        programs: &["freej2me", "freej2me-sdl", "freej2me-lwjgl"],
        jars: &[
            "freej2me.jar",
            "freej2me-sdl.jar",
            "freej2me-lwjgl.jar",
            "freej2me-awt.jar",
        ],
    },
    Known {
        name: "MicroEmulator",
        programs: &["microemulator"],
        jars: &["microemulator.jar", "microemu.jar"],
    },
    Known {
        name: "KEmulator",
        programs: &["kemulator", "KEmulator"],
        jars: &["KEmulator.jar", "kemulator.jar"],
    },
    Known {
        name: "J2ME SDK emulator",
        programs: &["emulator"],
        jars: &[],
    },
];

/// Everywhere this looks, named so the empty result can say so.
///
/// Returned rather than printed, because the caller decides how to say it and a list nobody can
/// see is not an explanation.
pub fn searched(home: Option<&Path>) -> Vec<PathBuf> {
    let mut places: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        places.extend(std::env::split_paths(&path));
    }
    for directory in jar_directories(home) {
        places.push(directory);
    }
    places
}

/// The places people actually keep an emulator jar.
fn jar_directories(home: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = home {
        for relative in [
            "",
            "Downloads",
            "Desktop",
            "emulators",
            "Emulators",
            "Apps",
            ".local/share",
            ".local/share/j2me",
            "j2me",
        ] {
            out.push(if relative.is_empty() {
                home.to_path_buf()
            } else {
                home.join(relative)
            });
        }
    }
    for absolute in ["/opt", "/usr/share/java", "/usr/local/share/java"] {
        out.push(PathBuf::from(absolute));
    }
    out
}

/// Emulators this machine appears to have.
///
/// `home` is a parameter rather than read from the environment so a test can point the search at a
/// directory it built, which is the only way to know the search works.
pub fn find(home: Option<&Path>) -> Vec<Found> {
    let mut found = Vec::new();

    for known in KNOWN {
        for program in known.programs {
            if let Some(path) = on_path(program) {
                found.push(Found {
                    name: known.name,
                    emulator: Emulator {
                        command: path.display().to_string(),
                        args: Vec::new(),
                    },
                    evidence: format!("{program} is on PATH"),
                    path,
                });
            }
        }
    }

    // A jar needs a JVM in front of it, so there is no point offering one where there is no java.
    let java = on_path("java");
    if let Some(java) = &java {
        for directory in jar_directories(home) {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                for known in KNOWN {
                    if !known.jars.iter().any(|j| j.eq_ignore_ascii_case(&name)) {
                        continue;
                    }
                    let path = entry.path();
                    found.push(Found {
                        name: known.name,
                        emulator: Emulator {
                            command: java.display().to_string(),
                            // `{game}` last, because that is what these take: the jar to run.
                            args: vec!["-jar".into(), path.display().to_string(), "{game}".into()],
                        },
                        evidence: format!("{name} in {}", directory.display()),
                        path,
                    });
                }
            }
        }
    }

    found.sort_by(|a, b| a.name.cmp(b.name).then(a.path.cmp(&b.path)));
    found.dedup_by(|a, b| a.path == b.path);
    found
}

/// Whether a JVM is available at all, which every jar-based emulator needs.
pub fn java_available() -> bool {
    on_path("java").is_some()
}

/// Looks a program up on PATH, the way a shell would.
///
/// No `which`: shelling out to find out whether something can be shelled out to is a dependency on
/// a program that may itself be missing, and the answer here is a file that exists and can be run.
fn on_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        for candidate in [
            directory.join(program),
            directory.join(format!("{program}.exe")),
            directory.join(format!("{program}.bat")),
        ] {
            if candidate.is_file() && executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn executable(_path: &Path) -> bool {
    // On Windows the extension decides, and the candidates above already carried it.
    true
}
