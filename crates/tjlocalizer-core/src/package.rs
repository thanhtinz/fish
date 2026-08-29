//! What kind of game this is, and how far this tool can take it.
//!
//! The project began with J2ME JARs and the rest of the world does not ship those. An Android
//! game is an APK, an iOS one an IPA, a PC game a folder or a zip - and all of those are,
//! underneath, either a ZIP archive or a directory of files. So the archive layer already reads
//! them; what was missing was knowing which one is in front of you and being honest about what
//! can then be done with it.
//!
//! The honesty is the point of this module. Reading text out of an APK is straightforward.
//! Putting it back is not: an Android package is signed, and an archive rewritten by this tool no
//! longer matches its signature, so the device refuses to install it. Re-signing needs a key that
//! belongs to a person, not to a program. Saying that up front is worth more than a build that
//! produces a file which cannot be installed and does not say why.

use crate::jar::Archive;
use serde::{Deserialize, Serialize};

/// The kinds of package this build recognises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A J2ME MIDlet: the case this project was built for.
    Midlet,
    /// A desktop or server Java archive.
    JavaArchive,
    /// An Android package.
    Apk,
    /// An iOS application archive.
    Ipa,
    /// A zip of files, which is what a great many PC games are once unpacked.
    Zip,
    /// A directory of files, as a game installed from a store sits on disk.
    Directory,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Midlet => "J2ME MIDlet",
            Kind::JavaArchive => "Java archive",
            Kind::Apk => "Android package",
            Kind::Ipa => "iOS application archive",
            Kind::Zip => "zip of files",
            Kind::Directory => "directory",
        }
    }

    /// Whether this tool can produce an installable package of this kind.
    ///
    /// `false` is not "unsupported". The text can still be read, translated, exported and written
    /// back into a copy of the archive - what cannot be done is hand somebody a file their device
    /// will install, because that needs a signature this tool has no business holding.
    pub fn can_repackage(self) -> bool {
        matches!(
            self,
            Kind::Midlet | Kind::JavaArchive | Kind::Zip | Kind::Directory
        )
    }

    /// Why not, in a sentence, for the caller to show.
    pub fn repackaging_note(self) -> Option<&'static str> {
        match self {
            Kind::Apk => Some(
                "an Android package is signed, and rewriting it breaks the signature; the device \
                 will refuse to install the result until somebody re-signs it with their own key",
            ),
            Kind::Ipa => Some(
                "an iOS application is signed and provisioned for particular devices; rewriting \
                 it breaks both, and neither can be redone without an Apple developer identity",
            ),
            _ => None,
        }
    }
}

/// What was found, and what said so.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Detected {
    pub kind: Kind,
    /// What in the package said so, so a wrong answer can be argued with.
    pub evidence: Vec<String>,
    /// Entries holding text this build knows how to read, with the format of each.
    pub readable: Vec<ReadableResource>,
    /// Entries holding text this build can see but not read, and why.
    pub opaque: Vec<OpaqueResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadableResource {
    pub entry: String,
    pub format: String,
    pub fields: usize,
}

/// Something that certainly holds text, in a format this build cannot open.
///
/// Listed rather than ignored. A translator who cannot see that a game keeps half its dialogue in
/// `resources.arsc` will conclude the game is half translated when it is not.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpaqueResource {
    pub entry: String,
    pub reason: String,
}

/// Identifies a package from what is inside it.
pub fn detect(archive: &Archive) -> Detected {
    let names: Vec<&str> = archive.entries().iter().map(|e| e.name.as_str()).collect();
    let has = |needle: &str| names.contains(&needle);
    let any = |f: &dyn Fn(&str) -> bool| names.iter().any(|n| f(n));

    let mut evidence = Vec::new();

    let kind = if has("AndroidManifest.xml")
        && any(&|n| n.starts_with("classes") && n.ends_with(".dex"))
    {
        evidence.push("AndroidManifest.xml and a classes.dex".into());
        Kind::Apk
    } else if any(&|n| n.starts_with("Payload/") && n.contains(".app/")) {
        evidence.push("a Payload/*.app directory".into());
        Kind::Ipa
    } else if any(&|n| n.ends_with(".class")) {
        let midlet = archive
            .get("META-INF/MANIFEST.MF")
            .map(|e| String::from_utf8_lossy(&e.data).contains("MIDlet-1"))
            .unwrap_or(false);
        if midlet {
            evidence.push("a MIDlet-1 attribute in the manifest".into());
            Kind::Midlet
        } else {
            evidence.push("Java class files with no MIDlet attribute".into());
            Kind::JavaArchive
        }
    } else {
        evidence.push("no class files, dex or app bundle".into());
        Kind::Zip
    };

    let (readable, opaque) = survey(archive);
    Detected {
        kind,
        evidence,
        readable,
        opaque,
    }
}

/// Which entries hold text this build can read, and which hold text it cannot.
fn survey(archive: &Archive) -> (Vec<ReadableResource>, Vec<OpaqueResource>) {
    let mut readable = Vec::new();
    let mut opaque = Vec::new();

    for entry in archive.entries() {
        if let Some(reason) = known_opaque(&entry.name) {
            opaque.push(OpaqueResource {
                entry: entry.name.clone(),
                reason: reason.to_string(),
            });
            continue;
        }
        if entry.is_class() || !crate::encoding::looks_like_text(&entry.data) {
            continue;
        }
        let Some(candidate) = crate::encoding::best(&entry.data, 0.5) else {
            continue;
        };
        let text = encoding_rs::Encoding::for_label(candidate.label.as_bytes())
            .unwrap_or(encoding_rs::UTF_8)
            .decode(&entry.data)
            .0
            .into_owned();

        let format = crate::resource::detect(&entry.name, &text);
        let fields = crate::resource::read(format, &text).len();
        if fields > 0 {
            readable.push(ReadableResource {
                entry: entry.name.clone(),
                format: format.name().to_string(),
                fields,
            });
        }
    }
    readable.sort_by(|a, b| b.fields.cmp(&a.fields).then(a.entry.cmp(&b.entry)));
    (readable, opaque)
}

/// Files that certainly hold text this build cannot open yet.
///
/// Named individually rather than guessed at, and each says what it is. A translator who cannot
/// see that a game keeps half its dialogue somewhere unreadable will conclude the game is half
/// translated when it is not.
fn known_opaque(name: &str) -> Option<&'static str> {
    let lower = name.to_lowercase();
    if lower.ends_with(".dex") {
        return Some("Android bytecode: its string pool is readable in principle, not yet here");
    }
    if lower.ends_with("resources.arsc") {
        return Some("Android's compiled resource table, a binary format");
    }
    if lower.ends_with(".xml") && !lower.contains("androidmanifest") {
        // A packaged APK compiles its XML; a source tree or an unpacked one does not.
        return None;
    }
    if lower.ends_with(".locres") {
        return Some("Unreal Engine's compiled string table, a binary format");
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
