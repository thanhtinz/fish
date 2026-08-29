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
    /// `translate <language> <label>:` blocks, as Ren'Py generates them.
    Renpy,
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
            Format::Renpy => "renpy",
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
    // Ren'Py ships translations as generated `.rpy` files: a `translate <language> <label>:`
    // header over the lines to fill in. Kept beside gettext because they are the same kind of
    // thing - a generated template carrying the original beside an empty slot - and the next
    // person looking for that pattern should find both in one place.
    //
    // The header is confirmed rather than the extension, because a `.rpy` without one is the
    // game's own script: there the dialogue is the original and the file is code, not a resource.
    let looks_like_renpy = lower.ends_with(".rpy") || lower.ends_with(".rpym");
    if looks_like_renpy && text.lines().any(|l| renpy_header(l.trim()).is_some()) {
        return Format::Renpy;
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
        Format::Renpy => read_renpy(text),
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
        Format::Renpy => write_renpy(text, patches),
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

// -- Ren'Py -----------------------------------------------------------------------------------

/// How a Ren'Py block label and the value inside it are written as one address.
///
/// The same `::` Unreal's string table uses, chosen for the same reason - a Ren'Py label is an
/// identifier and never contains it - but a constant of its own rather than a shared one. These
/// addresses are hashed into node ids and stored in every translation file, so borrowing
/// `locres::SEPARATOR` would mean an Unreal-motivated change to it silently orphaning every
/// Ren'Py translation on disk.
const RENPY_SEPARATOR: &str = "::";

/// Statements that look like a say statement and are not.
///
/// A generated dialogue block copies more than speech into itself - the voice line that goes with
/// a say, a `pause`, an `nvl clear`. They take a quoted argument and a speaker prefix is optional,
/// so nothing about their shape distinguishes them; the keyword does. Counting one of them as a
/// line of dialogue would shift every address after it in the block.
const RENPY_NOT_SAY: &[&str] = &[
    "voice", "play", "queue", "stop", "pause", "window", "nvl", "show", "hide", "scene", "with",
    "jump", "call", "return", "python", "init", "label", "old", "new",
];

/// One value in a generated Ren'Py translation file, and the line its target sits on.
///
/// Produced by one scan that both the reader and the writer call, for the reason `android_spans`
/// exists: the address of a dialogue line is its position in its block, and two walks that each
/// counted for themselves would eventually disagree about that position - which does not fail
/// loudly, it writes a translation into the line below the one it belongs to.
struct RenpyUnit {
    /// `<block label>::<discriminator>`; see `renpy_address`.
    key: String,
    /// The original, from the `# ...` comment or from the `old` line.
    source: String,
    /// Which line, in `text.lines()`, holds the string to be rewritten.
    target_line: usize,
}

/// The address of one value, as the rest of the pipeline uses it.
///
/// One rule for both kinds of block: a dialogue block gives its label and the position of the
/// line within it, the strings block gives the literal label `strings` and the original text. A
/// dialogue label cannot be `strings`, because Ren'Py's own parser would read the header it
/// generated as the strings block.
///
/// The address is only ever built, never split, so a `::` inside an original string cannot be
/// misread. Do not add a reverse of this function.
fn renpy_address(label: &str, discriminator: &str) -> String {
    format!("{label}{RENPY_SEPARATOR}{discriminator}")
}

/// The label of a `translate <language> <label>:` header, if the line is one.
///
/// Three words and a colon, exactly. A looser test matches ordinary prose - and this is also what
/// `detect` confirms a Ren'Py file by, so it is the one thing here that has to be tight. It is
/// not tight enough to reject `translate this label:` written as prose; reaching it at all
/// requires the `.rpy` extension, and that is deliberate rather than overlooked.
fn renpy_header(trimmed: &str) -> Option<String> {
    let head = trimmed.strip_prefix("translate ")?.strip_suffix(':')?;
    let mut words = head.split_whitespace();
    let _language = words.next()?;
    let label = words.next()?;
    words.next().is_none().then(|| label.to_string())
}

/// Splits `e "Hello."`, `"Just text."` or `e happy "Hello." nointeract` into the speaker prefix
/// and the quoted text.
///
/// The prefix is whatever stands before the quote, unparsed: a say statement may carry image
/// attributes, and all this has to do with the prefix is compare it with the one on the comment
/// above. Narration has no speaker, and an empty prefix is the ordinary case in a visual novel
/// rather than a failure.
fn renpy_say(trimmed: &str) -> Option<(String, String)> {
    let open = trimmed.find('"')?;
    let who = trimmed[..open].trim_end().to_string();
    let mut words = who.split_whitespace();
    if let Some(first) = words.clone().next() {
        if RENPY_NOT_SAY.contains(&first) {
            return None;
        }
    }
    // A speaker and its attributes are identifiers. Anything else before the quote - an operator,
    // a bracket, a comma - means this is some other statement that happens to take a string.
    if !words.all(|word| {
        word.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '@' | '-' | '.'))
    }) {
        return None;
    }
    let (text, _) = quoted(&trimmed[open + 1..])?;
    Some((who, text))
}

/// The string after `old ` or `new `, in the file's own escaped form.
fn renpy_quoted(rest: &str) -> Option<String> {
    let rest = rest.trim().strip_prefix('"')?;
    quoted(rest).map(|(text, _)| text)
}

fn renpy_units(text: &str) -> Vec<RenpyUnit> {
    let mut units = Vec::new();
    // Which block we are in, if any: `Some("strings")`, `Some("start_a1b2c3")`, or nothing.
    let mut block: Option<String> = None;
    // How many say statements this dialogue block has held so far. Reset with the block.
    let mut spoken = 0usize;
    // The speaker and the original read out of the comment above a statement not yet reached.
    let mut pending: Option<(String, String)> = None;

    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();

        // Ren'Py is indentation-sensitive, and that is what says where a block ends: any non-blank
        // line in column zero closes the one before it. This is also the first of two reasons the
        // `# game/script.rpy:12` note the generator writes above each header is never mistaken for
        // dialogue - it sits in column zero, so it closes a block rather than being read inside
        // one.
        if !trimmed.is_empty() && !line.starts_with([' ', '\t']) {
            block = renpy_header(trimmed);
            spoken = 0;
            pending = None;
            continue;
        }

        let Some(label) = block.as_deref() else {
            continue;
        };

        if label == "strings" {
            // `old "..."` over `new ""`. The same shape as gettext's msgid over msgstr, and read
            // the same way: the key is the original, because the file carries both.
            if let Some(rest) = trimmed.strip_prefix("old ") {
                pending = renpy_quoted(rest).map(|source| (String::new(), source));
            } else if trimmed.starts_with("new ") {
                if let Some((_, source)) = pending.take() {
                    if !source.is_empty() {
                        units.push(RenpyUnit {
                            key: renpy_address(label, &source),
                            source,
                            target_line: i,
                        });
                    }
                }
            }
            continue;
        }

        // A dialogue block. The original is in a comment; the translation goes in the statement
        // below it.
        if let Some(rest) = trimmed.strip_prefix('#') {
            // The second reason the file-and-line note is safe, and the one that covers the
            // strings block, where `# game/script.rpy:20` really is indented: `renpy_say` needs a
            // quoted string, and no file-and-line note has one. That, not the position in the
            // block, is what tells the two kinds of comment apart - a translator handed a file
            // path as a line of dialogue would translate it.
            pending = renpy_say(rest.trim());
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }

        let Some((who, _)) = renpy_say(trimmed) else {
            // Not a statement this reader understands. A dialogue block holds ordinary Ren'Py in
            // among the say statements, and guessing at it is how a file gets corrupted.
            pending = None;
            continue;
        };

        match pending.take() {
            // The speaker on the comment and the speaker on the statement have to match. Where
            // they do not, the pairing is not the one the generator wrote, and writing a
            // translation into it would put one character's line in another's mouth.
            Some((said_by, source)) if said_by == who && !source.is_empty() => {
                units.push(RenpyUnit {
                    key: renpy_address(label, &spoken.to_string()),
                    source,
                    target_line: i,
                });
                spoken += 1;
            }
            // Still a say statement, so it still holds a position in the block - it is only one
            // this reader has nothing to put in.
            _ => spoken += 1,
        }
    }
    units
}

/// Every line waiting for a translation, from both kinds of block.
///
/// A generated Ren'Py file carries the original beside the empty slot for the translation, the
/// way a `.po` file does - so what is read out is the original, and whatever is already in the
/// slot is left for `write` to overwrite or keep.
fn read_renpy(text: &str) -> Vec<Field> {
    renpy_units(text)
        .into_iter()
        .map(|unit| Field {
            key: unit.key,
            value: unit.source,
        })
        .collect()
}

fn write_renpy(text: &str, patches: &BTreeMap<String, String>) -> String {
    let mut out: Vec<String> = text.lines().map(|l| l.to_string()).collect();

    for unit in renpy_units(text) {
        let Some(target) = patches.get(&unit.key) else {
            continue;
        };
        let Some(line) = out.get_mut(unit.target_line) else {
            continue;
        };
        if let Some(rewritten) = replace_quoted(line, &escape_renpy(target)) {
            *line = rewritten;
        }
    }
    finish(text, out)
}

/// Replaces the first quoted string in a line, keeping every byte outside the quotes.
///
/// Both ends matter. Ren'Py decides what a line belongs to by its indentation, so a rebuilt line
/// that lost a space changes the structure of the file; and a say statement may carry clauses
/// after its string - `nointeract`, `with vpunch`, an `id` - that a rebuilt line would drop. That
/// is why this does not use the rebuild-from-the-indent idiom the other writers here use: their
/// lines have nothing after the string, and a say statement does.
fn replace_quoted(line: &str, value: &str) -> Option<String> {
    let open = line.find('"')?;
    let (_, rest) = quoted(&line[open + 1..])?;
    Some(format!("{}\"{value}\"{rest}", &line[..open]))
}

/// The characters Ren'Py recognises after a backslash.
///
/// `%`, `{` and `[` are in the set because escaping those is how a literal percent, brace or
/// bracket is written in text that also carries tags and interpolation; the space is Ren'Py's
/// non-collapsing space.
const RENPY_ESCAPES: &str = "\\\"'nt %{[";

/// Escapes a translation for a Ren'Py string, without escaping what is already escaped.
///
/// `read` hands back the file's own escaped form - `quoted` keeps backslashes - so a translator
/// who left `\"` or `\n` in place gives them back that way. Escaping blindly would turn `\n` into
/// a backslash and an `n` on the player's screen, and would do it again on every build, one level
/// deeper each time. Applying this twice gives the same result as applying it once, and that
/// property is what makes rebuilding an already-built game safe.
fn escape_renpy(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.peek() {
                Some(&next) if RENPY_ESCAPES.contains(next) => {
                    out.push('\\');
                    out.push(next);
                    chars.next();
                }
                // A backslash that means a backslash.
                _ => out.push_str("\\\\"),
            },
            '"' => out.push_str("\\\""),
            // A translation that arrived with a real line break still has to reach the file as
            // one Ren'Py can read: a raw newline inside a string ends the statement, and takes
            // the rest of the block with it.
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
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
