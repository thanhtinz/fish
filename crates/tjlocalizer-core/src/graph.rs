//! The content graph: every piece of text in a game, with enough information to patch it back.
//!
//! Extraction is only half the job. A node that cannot be written back is useless, so each one
//! carries an exact source location - which class and which constant pool index, or which
//! resource and which key - rather than just the text.

use crate::classfile::ClassFile;
use crate::encoding;
use crate::jar::Archive;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Exactly where a piece of text came from, and therefore where it goes back.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextSource {
    /// A string literal in a class constant pool.
    ClassConstant {
        class: String,
        utf8_index: u16,
        string_index: u16,
    },
    /// A `key=value` line in a properties-style resource.
    ResourceProperty { resource: String, key: String },
    /// A whole line of a plain text resource.
    ResourceLine { resource: String, line: usize },
}

/// What the text is used for. Drives translation style and whether it may be translated at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextType {
    /// Short interface labels: buttons, menu entries.
    Ui,
    /// Character speech.
    Dialogue,
    Quest,
    Item,
    Skill,
    /// Notices and errors from the game itself.
    System,
    Tutorial,
    Story,
    /// Carries placeholders that must survive translation.
    Format,
    /// A resource path, class name or similar. Must never be translated.
    Technical,
    Unknown,
}

impl ContextType {
    /// Whether a translator should be offered this node at all.
    pub fn is_translatable(self) -> bool {
        !matches!(self, ContextType::Technical)
    }
}

/// Limits a translation has to respect.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Constraints {
    /// Placeholders present in the source, which must all appear in the translation.
    pub placeholders: Vec<String>,
    /// Length of the original in characters, as a rough budget for layout.
    pub source_len: usize,
}

/// One translatable (or deliberately untranslatable) piece of text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextNode {
    /// Stable across runs: derived from the source location and original bytes, so re-analysing
    /// an unchanged game reproduces the same ids and existing translations still match.
    pub id: String,
    pub source: TextSource,
    pub source_text: String,
    /// Set when the bytes were not valid modified UTF-8 and had to be decoded another way.
    pub source_encoding: Option<String>,
    pub context: ContextType,
    pub constraints: Constraints,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContentGraph {
    pub nodes: Vec<TextNode>,
}

impl ContentGraph {
    pub fn translatable(&self) -> impl Iterator<Item = &TextNode> {
        self.nodes.iter().filter(|n| n.context.is_translatable())
    }

    pub fn get(&self, id: &str) -> Option<&TextNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// Walks an archive and builds its content graph.
pub fn extract(archive: &Archive) -> ContentGraph {
    let mut nodes = Vec::new();

    for entry in archive.classes() {
        let Ok(class) = ClassFile::parse(&entry.data) else {
            // A class that will not parse is reported by validation, not silently repaired here.
            continue;
        };
        for literal in class.string_literals() {
            let (text, source_encoding) = match &literal.decoded {
                Some(text) => (text.clone(), None),
                None => {
                    // Not modified UTF-8: the game stores this in its own charset. Offer the best
                    // guess so it is at least visible, and record which charset produced it.
                    match encoding::best(&literal.raw, 0.5) {
                        Some(candidate) => {
                            let (decoded, _, _) = encoding_rs::Encoding::for_label(
                                candidate.label.as_bytes(),
                            )
                            .unwrap_or(encoding_rs::WINDOWS_1252)
                            .decode(&literal.raw);
                            (decoded.into_owned(), Some(candidate.label))
                        }
                        None => continue,
                    }
                }
            };
            if text.trim().is_empty() {
                continue;
            }

            let source = TextSource::ClassConstant {
                class: entry.name.clone(),
                utf8_index: literal.utf8_index,
                string_index: literal.string_index,
            };
            nodes.push(make_node(source, text, source_encoding));
        }
    }

    for entry in archive.entries() {
        if entry.is_class() || is_archive_metadata(&entry.name) || !encoding::looks_like_text(&entry.data) {
            continue;
        }
        let Some(candidate) = encoding::best(&entry.data, 0.5) else {
            continue;
        };
        let decoded = encoding_rs::Encoding::for_label(candidate.label.as_bytes())
            .unwrap_or(encoding_rs::UTF_8)
            .decode(&entry.data)
            .0
            .into_owned();

        let is_properties = entry.extension() == "properties";
        for (index, line) in decoded.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
                continue;
            }
            let (source, text) = if is_properties {
                match trimmed.split_once('=') {
                    Some((key, value)) if !value.trim().is_empty() => (
                        TextSource::ResourceProperty {
                            resource: entry.name.clone(),
                            key: key.trim().to_string(),
                        },
                        value.trim().to_string(),
                    ),
                    _ => continue,
                }
            } else {
                (
                    TextSource::ResourceLine {
                        resource: entry.name.clone(),
                        line: index,
                    },
                    trimmed.to_string(),
                )
            };
            nodes.push(make_node(source, text, Some(candidate.label.clone())));
        }
    }

    ContentGraph { nodes }
}

/// Archive structure that must never be offered as game text.
///
/// The manifest and JAD are text files, so a plain "does this decode?" test happily hands a
/// translator lines like `MIDlet-1: Sample Game,/icon.png,SampleGame` - and translating one
/// renames the entry point, breaking a game that then installs and refuses to start. These files
/// are the archive's structure and are written by the manifest code path, not the text one.
/// Signature files are excluded for the same reason: they describe the archive, not the game.
///
/// This is a rule about the JAR format, not about any particular game, which is why it belongs
/// here rather than in a profile.
fn is_archive_metadata(name: &str) -> bool {
    let upper = name.to_uppercase();
    upper.starts_with("META-INF/")
        || upper.ends_with(".MF")
        || upper.ends_with(".JAD")
        || upper.ends_with(".SF")
}

fn make_node(source: TextSource, text: String, source_encoding: Option<String>) -> TextNode {
    let context = classify(&text);
    let placeholders = find_placeholders(&text);
    TextNode {
        id: node_id(&source, &text),
        constraints: Constraints {
            placeholders,
            source_len: text.chars().count(),
        },
        source_text: text,
        source_encoding,
        context,
        source,
    }
}

/// A stable id for a node.
///
/// Hashing the location together with the original text means an unchanged game re-analyses to
/// the same ids, so approved translations survive a re-import. It also means that if the source
/// text at a location changes, the id changes and the stale translation is not silently reused.
fn node_id(source: &TextSource, text: &str) -> String {
    let mut hasher = Sha256::new();
    match source {
        TextSource::ClassConstant { class, utf8_index, .. } => {
            hasher.update(b"class\0");
            hasher.update(class.as_bytes());
            hasher.update(utf8_index.to_be_bytes());
        }
        TextSource::ResourceProperty { resource, key } => {
            hasher.update(b"prop\0");
            hasher.update(resource.as_bytes());
            hasher.update(key.as_bytes());
        }
        TextSource::ResourceLine { resource, line } => {
            hasher.update(b"line\0");
            hasher.update(resource.as_bytes());
            hasher.update(line.to_be_bytes());
        }
    }
    hasher.update(b"\0");
    hasher.update(text.as_bytes());
    hasher
        .finalize()
        .iter()
        .take(12)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Placeholder tokens that must survive translation unchanged.
///
/// Losing one turns a runtime format call into a crash or a visibly broken line, so they are
/// recorded at extraction and checked again at validation.
pub fn find_placeholders(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            // printf-style: %d %s %02d %%
            '%' if i + 1 < bytes.len() => {
                let start = i;
                let mut j = i + 1;
                while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == '.') {
                    j += 1;
                }
                if j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                    out.push(bytes[start..=j].iter().collect());
                    i = j + 1;
                    continue;
                }
                if j < bytes.len() && bytes[j] == '%' {
                    i = j + 1;
                    continue;
                }
                i += 1;
            }
            // MessageFormat-style: {0} {name}
            '{' => {
                if let Some(end) = bytes[i..].iter().position(|&c| c == '}') {
                    let token: String = bytes[i..=i + end].iter().collect();
                    if token.len() <= 24 {
                        out.push(token);
                    }
                    i += end + 1;
                    continue;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// Classifies a string by what it is for (specification §10).
///
/// Every rule here is about the *shape* of the text, never about a particular game. The most
/// important job is recognising technical strings: translating a resource path or a class name
/// breaks the game, and unlike a clumsy translation it fails silently until that code runs.
pub fn classify(text: &str) -> ContextType {
    let trimmed = text.trim();

    if looks_technical(trimmed) {
        return ContextType::Technical;
    }
    if !find_placeholders(trimmed).is_empty() {
        return ContextType::Format;
    }

    let words = trimmed.split_whitespace().count();
    let chars = trimmed.chars().count();
    let ends_sentence = trimmed.ends_with(['.', '!', '?', '…', '。', '！', '？']);

    if chars <= 16 && words <= 2 && !ends_sentence {
        return ContextType::Ui;
    }
    if ends_sentence && words >= 4 {
        return ContextType::Dialogue;
    }
    if words >= 3 {
        return ContextType::Story;
    }
    ContextType::Unknown
}

/// Detects strings that are structural rather than displayable.
fn looks_technical(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }
    // Resource paths and file names.
    if text.starts_with('/') || text.starts_with("./") {
        return true;
    }
    // Any slash-separated token with no spaces: a path, or an internal class name like
    // "com/example/Main". Display text containing a slash - "HP: 10 / 20" - has spaces around
    // it. Without this rule a class name is classified as a short UI label and offered for
    // translation, which silently breaks the class that references it.
    if text.contains('/') && !text.contains(' ') {
        return true;
    }
    // Fully-qualified class names and descriptors.
    if text.starts_with('L') && text.ends_with(';') && text.contains('/') {
        return true;
    }
    if text.contains("()") || text.starts_with('(') {
        return true;
    }
    // A bare token with no spaces that carries an extension.
    if !text.contains(' ') {
        if let Some((_, ext)) = text.rsplit_once('.') {
            if (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
                return true;
            }
        }
    }
    false
}
