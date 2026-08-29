//! Whether a resource can be written back, and how.
//!
//! One question, asked in one place, by everything that needs it: the extractor deciding how to
//! read a file, the build deciding how to patch it, and the interface deciding whether to promise
//! a translator that their work will reach the game.
//!
//! It exists because those three had begun to disagree. Extraction recognised Unreal's string
//! table by its file extension and swallowed a parse failure; the package survey recognised it by
//! its magic bytes and reported one. So `analyze` could say a file held three readable strings
//! while `extract` produced none from it, and nothing on screen explained the difference. With one
//! binary format that is survivable. With four it is a bug waiting for its turn.
//!
//! The rule here is **default deny**. Bytes that do not read as text and match no writer this
//! build owns are `ReadOnly`, and a `ReadOnly` resource is never touched. That is not caution for
//! its own sake: before this module existed, the build decoded every patched resource with
//! `from_utf8_lossy` and wrote it back. Nothing had a node in a binary file yet, so nothing broke -
//! but the first reader for Android bytecode would have turned every invalid byte of a `classes.dex`
//! into U+FFFD, written the result back, and reported success.

use crate::resource::Format;

/// The binary formats this build can read, and whether it can write them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryFormat {
    /// Unreal Engine's compiled string table. Read and written.
    Locres,
}

impl BinaryFormat {
    pub fn name(self) -> &'static str {
        match self {
            BinaryFormat::Locres => "unreal-locres",
        }
    }
}

/// What can be done with one resource.
#[derive(Debug, Clone, PartialEq)]
pub enum Plan {
    /// Text, edited in place through `resource::write`.
    Text { format: Format, encoding: String },
    /// A binary format with a reader and a writer of its own.
    Binary(BinaryFormat),
    /// Readable or not, but not writable by this build. The reason reaches the person verbatim.
    ReadOnly { reason: String },
}

impl Plan {
    pub fn writable(&self) -> bool {
        !matches!(self, Plan::ReadOnly { .. })
    }

    /// What to call this format on screen.
    pub fn format_name(&self) -> &str {
        match self {
            Plan::Text { format, .. } => format.name(),
            Plan::Binary(binary) => binary.name(),
            Plan::ReadOnly { .. } => "read-only",
        }
    }
}

/// Decides what can be done with one resource, from its name and its bytes.
///
/// Both are used. The name alone is not enough - games ship JSON in `.txt` files, and a `.locres`
/// renamed is still a `.locres` - and the bytes alone are not either, since a gettext catalogue
/// and a properties file both look like lines with an equals sign in them.
pub fn plan(entry_name: &str, data: &[u8]) -> Plan {
    plan_with(entry_name, data, &crate::plugin::Formats::default())
}

/// The same question, asked where a plugin may have claimed the file (§20).
///
/// A plugin claim is consulted after the binary readers and before the text detectors: a plugin
/// exists to name a file whose *shape* nothing here recognises - a game's `data/lang/en.txt` that
/// is really a properties file - and it has no business overruling a reader that parsed the bytes
/// and knows what they are. What it does overrule is the guess, which is the part that was wrong.
pub fn plan_with(entry_name: &str, data: &[u8], formats: &crate::plugin::Formats) -> Plan {
    if crate::locres::Locres::looks_like(data) {
        return match crate::locres::Locres::parse(data) {
            Ok(_) => Plan::Binary(BinaryFormat::Locres),
            // A version of the format this build does not read. Saying which is the difference
            // between "there is nothing here" and "there is something here I cannot open".
            Err(e) => Plan::ReadOnly {
                reason: e.to_string(),
            },
        };
    }

    if let Some(reason) = unwritable_binary(entry_name) {
        return Plan::ReadOnly {
            reason: reason.to_string(),
        };
    }

    if !crate::encoding::looks_like_text(data) {
        return Plan::ReadOnly {
            reason: "not text, and no reader here recognises it".into(),
        };
    }
    let Some(candidate) = crate::encoding::best(data, 0.5) else {
        return Plan::ReadOnly {
            reason: "its bytes decode as no character set this build knows".into(),
        };
    };
    let text = decode(data, &candidate.label);
    if let Some(claim) = formats.of(entry_name) {
        return Plan::Text {
            format: claim.format,
            encoding: candidate.label,
        };
    }
    let format = crate::resource::detect(entry_name, &text);

    // A `.rpy` that `detect` could not place is the game's own script, not a resource - and it
    // has to be named as read-only here rather than left to fall through, because the fallback is
    // worse than no support at all. `Lines` offers every non-blank line of the file as
    // translatable, `label start:` and `$ points += 1` included, and writes back by replacing the
    // whole line. One approved line and the game stops parsing.
    //
    // Ren'Py is localized through the files its own tooling generates under `game/tl/`, which is
    // what the branch above recognises.
    if format == crate::resource::Format::Lines && is_renpy_source(entry_name) {
        return Plan::ReadOnly {
            reason: "a Ren'Py script: Ren'Py is translated through the generated files under \
                     game/tl/, and rewriting the script itself is not how that works"
                .into(),
        };
    }

    Plan::Text {
        format,
        encoding: candidate.label,
    }
}

/// Whether a name is a Ren'Py source file, as opposed to a generated translation file.
///
/// Both end in `.rpy`; only the contents tell them apart, which is why this is a name test used
/// after `resource::detect` has looked at the contents rather than before.
fn is_renpy_source(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".rpy") || lower.ends_with(".rpym")
}

/// Decodes a resource with the character set that was detected for it.
pub fn decode(data: &[u8], label: &str) -> String {
    encoding_rs::Encoding::for_label(label.as_bytes())
        .unwrap_or(encoding_rs::UTF_8)
        .decode(data)
        .0
        .into_owned()
}

/// Binary formats this build knows of and cannot write, named individually.
///
/// Named rather than lumped together as "binary", because the three states a person needs to tell
/// apart are "this holds text you can change", "this holds text you cannot change yet", and "this
/// is not text". A file in the middle state is a piece of the game's language that will not be
/// translated, and a translator who cannot see which files those are will read a half-translated
/// game as a finished one.
fn unwritable_binary(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if lower.ends_with(".dex") {
        return Some("Android bytecode: its string pool is readable in principle, not yet here");
    }
    if lower.ends_with("resources.arsc") {
        return Some("Android's compiled resource table, a binary format");
    }
    if lower.ends_with(".assets") || lower.ends_with(".bundle") || lower.ends_with(".unity3d") {
        return Some("a Unity asset bundle, which needs its own reader");
    }
    if lower.ends_with(".pck") {
        return Some("a Godot package, which needs its own reader");
    }
    if lower.ends_with(".rpa") {
        return Some("a Ren'Py archive, which needs its own reader");
    }
    None
}
