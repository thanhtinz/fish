//! Text resources that are not Java classes.
//!
//! A J2ME game keeps its text in the constant pool and in `key=value` files, and for a long time
//! that was all this project needed to read. It is not what the rest of the world ships. An
//! Android game keeps strings in `res/values/strings.xml`, an iOS one in `Localizable.strings`, a
//! Unity or RPG Maker game in JSON, and a great many PC games in gettext `.po` files.
//!
//! All of those are the same shape underneath - a key, a value, and a lot of surrounding text
//! that must survive untouched - so they are one module rather than five special cases in the
//! extractor. The surviving-untouched part is the whole difficulty: a file rewritten from a
//! parsed model loses its comments, its ordering and its formatting, and a game's own tooling
//! then reports a diff nobody made. So every writer here edits in place and leaves the rest of
//! the file alone.

use std::collections::BTreeMap;

/// The formats this build can read and write back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `key=value`, as J2ME and Java ship.
    Properties,
    /// `"key" = "value";`, as Apple ships.
    AppleStrings,
    /// `<string name="key">value</string>`, as Android ships.
    AndroidStrings,
    /// gettext `msgid`/`msgstr`, as a great many PC games ship.
    Gettext,
    /// `[section]` headings over `key=value`, as older PC games ship.
    Ini,
    /// A JSON object or array of them, as RPG Maker and many engines ship.
    Json,
    /// Nothing recognised: every non-empty line is offered on its own.
    Lines,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::Properties => "properties",
            Format::AppleStrings => "apple-strings",
            Format::AndroidStrings => "android-strings",
            Format::Gettext => "gettext",
            Format::Ini => "ini",
            Format::Json => "json",
            Format::Lines => "lines",
        }
    }

    /// Whether values in this format are addressed by key rather than by line number.
    ///
    /// A key survives the file being edited; a line number does not. Where a format has keys they
    /// are used, so a translation still matches after somebody adds a line to the file.
    pub fn keyed(self) -> bool {
        self != Format::Lines
    }
}

/// What format a resource is in, decided by its name and its contents.
///
/// The name alone is not enough - plenty of games ship `.txt` files full of JSON - and the
/// contents alone are not either, since a gettext file and a properties file both look like lines
/// with an equals sign in them. Both are used, and neither is trusted on its own.
pub fn detect(name: &str, text: &str) -> Format {
    let lower = name.to_lowercase();
    let trimmed = text.trim_start();

    let looks_like_json =
        lower.ends_with(".json") || trimmed.starts_with('{') || trimmed.starts_with('[');
    if looks_like_json && serde_json::from_str::<serde_json::Value>(text).is_ok() {
        return Format::Json;
    }
    let looks_like_gettext =
        lower.ends_with(".po") || lower.ends_with(".pot") || text.contains("\nmsgstr");
    if looks_like_gettext && text.contains("msgid") {
        return Format::Gettext;
    }
    if lower.ends_with(".strings") {
        return Format::AppleStrings;
    }
    if lower.ends_with(".xml") && text.contains("<string") {
        return Format::AndroidStrings;
    }
    if lower.ends_with(".properties") {
        return Format::Properties;
    }
    // A section heading is what separates an INI from a properties file, and it matters: two
    // sections may hold the same key, and a reader that ignored sections would translate one and
    // silently overwrite the other.
    if (lower.ends_with(".ini") || lower.ends_with(".cfg") || lower.ends_with(".txt"))
        && text.lines().any(|l| {
            let t = l.trim();
            t.starts_with('[') && t.ends_with(']') && t.len() > 2
        })
        && text.contains('=')
    {
        return Format::Ini;
    }
    Format::Lines
}

/// One translatable value, and how to find it again.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// The key, or the line number as text where the format has no keys.
    pub key: String,
    pub value: String,
}

/// Every translatable value in a resource.
pub fn read(format: Format, text: &str) -> Vec<Field> {
    match format {
        Format::Properties => read_properties(text),
        Format::AppleStrings => read_apple(text),
        Format::AndroidStrings => read_android(text),
        Format::Gettext => read_gettext(text),
        Format::Ini => read_ini(text),
        Format::Json => read_json(text),
        Format::Lines => read_lines(text),
    }
}

/// Rewrites the values named in `patches`, leaving everything else exactly as it was.
pub fn write(format: Format, text: &str, patches: &BTreeMap<String, String>) -> String {
    match format {
        Format::Properties => write_properties(text, patches),
        Format::AppleStrings => write_apple(text, patches),
        Format::AndroidStrings => write_android(text, patches),
        Format::Gettext => write_gettext(text, patches),
        Format::Ini => write_ini(text, patches),
        Format::Json => write_json(text, patches),
        Format::Lines => write_lines(text, patches),
    }
}

// -- properties -------------------------------------------------------------------------------

fn read_properties(text: &str) -> Vec<Field> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
                return None;
            }
            let (key, value) = trimmed.split_once('=')?;
            let value = value.trim();
            (!value.is_empty()).then(|| Field {
                key: key.trim().to_string(),
                value: value.to_string(),
            })
        })
        .collect()
}

fn write_properties(text: &str, patches: &BTreeMap<String, String>) -> String {
    rewrite_lines(text, |line| {
        let (key, _) = line.split_once('=')?;
        let target = patches.get(key.trim())?;
        Some(format!("{key}={target}"))
    })
}

// -- Apple .strings ---------------------------------------------------------------------------

/// `"key" = "value";`, with comments and blank lines around it.
fn read_apple(text: &str) -> Vec<Field> {
    text.lines()
        .filter_map(|line| {
            let (key, value) = apple_pair(line)?;
            (!value.is_empty()).then_some(Field { key, value })
        })
        .collect()
}

fn apple_pair(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('"') {
        return None;
    }
    let (key, rest) = quoted(&trimmed[1..])?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let (value, _) = quoted(rest.strip_prefix('"')?)?;
    Some((key, value))
}

/// Reads up to the closing quote, honouring backslash escapes.
fn quoted(text: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut chars = text.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                let (_, escaped) = chars.next()?;
                out.push('\\');
                out.push(escaped);
            }
            '"' => return Some((out, &text[i + 1..])),
            _ => out.push(c),
        }
    }
    None
}

fn write_apple(text: &str, patches: &BTreeMap<String, String>) -> String {
    rewrite_lines(text, |line| {
        let (key, _) = apple_pair(line)?;
        let target = patches.get(&key)?;
        let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
        // The trailing semicolon is part of the format, and a file missing one stops parsing at
        // that line - taking every string after it with it.
        Some(format!("{indent}\"{key}\" = \"{}\";", escape_apple(target)))
    })
}

fn escape_apple(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

// -- Android strings.xml ----------------------------------------------------------------------

/// `<string name="key">value</string>`, wherever it appears.
///
/// Scanned over the whole document rather than line by line. A hand-written `strings.xml` puts
/// one string per line and a line-based reader would do; a generated or minified one puts the
/// whole file on one, and a reader that only looked at line starts would silently find nothing in
/// it - which reads as "this file has no text in it" rather than as a limitation.
///
/// Multi-line strings and plural forms are left alone rather than half-handled.
fn read_android(text: &str) -> Vec<Field> {
    android_spans(text)
        .into_iter()
        .filter_map(|span| {
            let value = &text[span.value.clone()];
            (!value.trim().is_empty()).then(|| Field {
                key: text[span.key].to_string(),
                value: value.to_string(),
            })
        })
        .collect()
}

/// Where one `<string>` element's name and body sit in the document.
struct StringSpan {
    key: std::ops::Range<usize>,
    value: std::ops::Range<usize>,
}

fn android_spans(text: &str) -> Vec<StringSpan> {
    let mut spans = Vec::new();
    let mut from = 0usize;

    while let Some(found) = text[from..].find("<string ") {
        let tag = from + found;
        let rest = &text[tag..];
        from = tag + "<string ".len();

        let Some(name_at) = rest.find("name=\"") else {
            continue;
        };
        let key_from = tag + name_at + "name=\"".len();
        let Some(key_len) = text[key_from..].find('"') else {
            continue;
        };
        let Some(open) = text[key_from..].find('>') else {
            continue;
        };
        let value_from = key_from + open + 1;
        let Some(close) = text[value_from..].find("</string>") else {
            continue;
        };

        spans.push(StringSpan {
            key: key_from..key_from + key_len,
            value: value_from..value_from + close,
        });
        from = value_from + close;
    }
    spans
}

fn write_android(text: &str, patches: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut copied = 0usize;

    for span in android_spans(text) {
        let Some(target) = patches.get(&text[span.key.clone()]) else {
            continue;
        };
        out.push_str(&text[copied..span.value.start]);
        out.push_str(&escape_xml(target));
        copied = span.value.end;
    }
    out.push_str(&text[copied..]);
    out
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;")
}

// -- gettext ----------------------------------------------------------------------------------

/// `msgid "..."` followed by `msgstr "..."`.
///
/// The key is the msgid, which is also the source text - so a `.po` file already carries the
/// original alongside the translation, and reading one gives both.
fn read_gettext(text: &str) -> Vec<Field> {
    let mut fields = Vec::new();
    let mut pending: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("msgid ") {
            pending = gettext_quoted(rest).filter(|id| !id.is_empty());
        } else if trimmed.starts_with("msgstr ") {
            if let Some(key) = pending.take() {
                fields.push(Field {
                    value: key.clone(),
                    key,
                });
            }
        }
    }
    fields
}

fn gettext_quoted(rest: &str) -> Option<String> {
    let rest = rest.trim().strip_prefix('"')?;
    quoted(rest).map(|(text, _)| text)
}

fn write_gettext(text: &str, patches: &BTreeMap<String, String>) -> String {
    let mut out = Vec::new();
    let mut pending: Option<String> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("msgid ") {
            pending = gettext_quoted(rest);
            out.push(line.to_string());
            continue;
        }
        if trimmed.starts_with("msgstr ") {
            if let Some(target) = pending.take().and_then(|id| patches.get(&id)) {
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                out.push(format!("{indent}msgstr \"{}\"", escape_apple(target)));
                continue;
            }
        }
        out.push(line.to_string());
    }
    finish(text, out)
}

// -- INI --------------------------------------------------------------------------------------

/// `[section]` headings over `key=value`.
///
/// Keys are qualified by their section, because two sections may hold the same key and a reader
/// that ignored sections would translate one of them and silently overwrite the other.
fn read_ini(text: &str) -> Vec<Field> {
    let mut fields = Vec::new();
    let mut section = String::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = ini_section(trimmed) {
            section = name;
            continue;
        }
        if trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        fields.push(Field {
            key: ini_address(&section, key.trim()),
            value: value.to_string(),
        });
    }
    fields
}

fn ini_section(trimmed: &str) -> Option<String> {
    let inside = trimmed.strip_prefix('[')?.strip_suffix(']')?;
    (!inside.is_empty()).then(|| inside.to_string())
}

fn ini_address(section: &str, key: &str) -> String {
    if section.is_empty() {
        key.to_string()
    } else {
        format!("{section}.{key}")
    }
}

fn write_ini(text: &str, patches: &BTreeMap<String, String>) -> String {
    let mut section = String::new();
    let out: Vec<String> = text
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if let Some(name) = ini_section(trimmed) {
                section = name;
                return line.to_string();
            }
            if trimmed.starts_with('#') || trimmed.starts_with(';') {
                return line.to_string();
            }
            let Some((key, _)) = line.split_once('=') else {
                return line.to_string();
            };
            match patches.get(&ini_address(&section, key.trim())) {
                Some(target) => format!("{key}={target}"),
                None => line.to_string(),
            }
        })
        .collect();
    finish(text, out)
}

// -- JSON -------------------------------------------------------------------------------------

/// Every string in the document, keyed by its path.
///
/// Paths rather than names, because the shape of a game's JSON is the game's business: RPG Maker
/// keeps dialogue several arrays deep, and a flat key would collide the moment two objects used
/// the same field name.
fn read_json(text: &str) -> Vec<Field> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let mut fields = Vec::new();
    walk_json(&value, &mut String::new(), &mut fields);
    fields
}

fn walk_json(value: &serde_json::Value, path: &mut String, out: &mut Vec<Field>) {
    match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => out.push(Field {
            key: path.clone(),
            value: text.clone(),
        }),
        serde_json::Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                let was = path.len();
                path.push_str(&format!("[{i}]"));
                walk_json(item, path, out);
                path.truncate(was);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, item) in map {
                let was = path.len();
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(key);
                walk_json(item, path, out);
                path.truncate(was);
            }
        }
        _ => {}
    }
}

/// Rewrites the named strings and re-serialises.
///
/// The one format here that does not preserve its own formatting, because a JSON document has no
/// line structure worth preserving: a game reads it with a parser, and the file is a serialisation
/// rather than something a person maintains. Said plainly because it is the exception.
fn write_json(text: &str, patches: &BTreeMap<String, String>) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(text) else {
        return text.to_string();
    };
    let mut path = String::new();
    patch_json(&mut value, &mut path, patches);
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| text.to_string())
}

fn patch_json(
    value: &mut serde_json::Value,
    path: &mut String,
    patches: &BTreeMap<String, String>,
) {
    match value {
        serde_json::Value::String(text) => {
            if let Some(target) = patches.get(path.as_str()) {
                *text = target.clone();
            }
        }
        serde_json::Value::Array(items) => {
            for (i, item) in items.iter_mut().enumerate() {
                let was = path.len();
                path.push_str(&format!("[{i}]"));
                patch_json(item, path, patches);
                path.truncate(was);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, item) in map.iter_mut() {
                let was = path.len();
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(key);
                patch_json(item, path, patches);
                path.truncate(was);
            }
        }
        _ => {}
    }
}

// -- plain lines ------------------------------------------------------------------------------

fn read_lines(text: &str) -> Vec<Field> {
    text.lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let trimmed = line.trim();
            (!trimmed.is_empty() && !trimmed.starts_with('#')).then(|| Field {
                key: i.to_string(),
                value: trimmed.to_string(),
            })
        })
        .collect()
}

fn write_lines(text: &str, patches: &BTreeMap<String, String>) -> String {
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    for (key, target) in patches {
        if let Ok(index) = key.parse::<usize>() {
            if let Some(slot) = lines.get_mut(index) {
                *slot = target.clone();
            }
        }
    }
    finish(text, lines)
}

// -- shared -----------------------------------------------------------------------------------

/// Applies a per-line edit, keeping every line the edit declines to touch.
fn rewrite_lines(text: &str, mut edit: impl FnMut(&str) -> Option<String>) -> String {
    let out: Vec<String> = text
        .lines()
        .map(|line| edit(line).unwrap_or_else(|| line.to_string()))
        .collect();
    finish(text, out)
}

/// Joins lines back, keeping whether the file ended with a newline.
///
/// A file that gained or lost a trailing newline shows up as changed in every diff tool there is,
/// which is noise somebody has to rule out before they can see the real change.
fn finish(original: &str, lines: Vec<String>) -> String {
    let mut out = lines.join("\n");
    if original.ends_with('\n') {
        out.push('\n');
    }
    out
}
