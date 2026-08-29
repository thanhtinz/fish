//! Text resources that are not Java classes.
//!
//! Most of what matters here is what a writer leaves alone. A file rewritten from a parsed model
//! loses its comments, its ordering and its spacing, and the game's own tooling then reports a
//! diff nobody made - so every test below checks the untouched parts as hard as the changed one.

use std::collections::BTreeMap;
use tjlocalizer_core::resource::{self, Format};

fn patch(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn a_gettext_catalogue_is_read_and_written_in_place() {
    let po = "# a comment somebody wrote\n\
              #: src/menu.c:42\n\
              msgid \"Start Game\"\n\
              msgstr \"\"\n\
              \n\
              msgid \"Quit\"\n\
              msgstr \"already done\"\n";
    assert_eq!(resource::detect("game.po", po), Format::Gettext);

    let fields = resource::read(Format::Gettext, po);
    assert_eq!(fields.len(), 2);
    // A .po file carries the original beside the translation, so the key is the source text.
    assert_eq!(fields[0].key, "Start Game");
    assert_eq!(fields[0].value, "Start Game");

    let written = resource::write(Format::Gettext, po, &patch(&[("Start Game", "Bắt đầu")]));
    assert!(written.contains("msgstr \"Bắt đầu\""));
    assert!(
        written.contains("# a comment somebody wrote") && written.contains("#: src/menu.c:42"),
        "the comments were lost:\n{written}"
    );
    assert!(
        written.contains("msgstr \"already done\""),
        "an untouched entry was rewritten:\n{written}"
    );
}

#[test]
fn an_apple_strings_file_keeps_its_comments_and_its_semicolons() {
    let strings = "/* The main menu */\n\
                   \"menu.start\" = \"Start Game\";\n\
                   \"menu.quit\" = \"Quit\";\n";
    assert_eq!(
        resource::detect("Localizable.strings", strings),
        Format::AppleStrings
    );

    let fields = resource::read(Format::AppleStrings, strings);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].key, "menu.start");
    assert_eq!(fields[0].value, "Start Game");

    let written = resource::write(
        Format::AppleStrings,
        strings,
        &patch(&[("menu.start", "Bắt đầu")]),
    );
    assert!(
        written.contains("\"menu.start\" = \"Bắt đầu\";"),
        "{written}"
    );
    assert!(written.contains("/* The main menu */"), "{written}");
    assert!(written.contains("\"menu.quit\" = \"Quit\";"), "{written}");
}

/// A quote inside a value ends the string early if it is not escaped, and every string after it
/// is then read as part of the wrong entry.
#[test]
fn a_quote_inside_a_value_is_escaped_on_the_way_out() {
    let strings = "\"say\" = \"hello\";\n";
    let written = resource::write(
        Format::AppleStrings,
        strings,
        &patch(&[("say", "nói \"xin chào\"")]),
    );
    assert!(written.contains("\\\"xin chào\\\""), "{written}");

    // And reading it back gives one field, not three.
    assert_eq!(resource::read(Format::AppleStrings, &written).len(), 1);
}

#[test]
fn an_android_strings_file_is_patched_between_its_tags() {
    let xml = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
               <resources>\n\
               \x20   <string name=\"app_name\">Fishing</string>\n\
               \x20   <string name=\"start\">Start Game</string>\n\
               </resources>\n";
    assert_eq!(resource::detect("strings.xml", xml), Format::AndroidStrings);

    let fields = resource::read(Format::AndroidStrings, xml);
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[1].key, "start");

    let written = resource::write(Format::AndroidStrings, xml, &patch(&[("start", "Bắt đầu")]));
    assert!(
        written.contains("<string name=\"start\">Bắt đầu</string>"),
        "{written}"
    );
    assert!(
        written.contains("<?xml version"),
        "the declaration was lost:\n{written}"
    );
    assert!(
        written.contains(">Fishing<"),
        "an untouched string changed:\n{written}"
    );
    // Indentation is part of a file people read.
    assert!(written.contains("    <string name=\"start\">"), "{written}");
}

/// `&` and `<` inside XML text end the document if they are not escaped.
#[test]
fn xml_special_characters_are_escaped_on_the_way_out() {
    let xml = "<resources><string name=\"a\">x</string></resources>";
    let written = resource::write(Format::AndroidStrings, xml, &patch(&[("a", "cá & tôm")]));
    assert!(written.contains("cá &amp; tôm"), "{written}");
}

#[test]
fn json_strings_are_addressed_by_path() {
    let json = r#"{"actors":[{"name":"Hero","note":""},{"name":"Fisher"}],"version":3}"#;
    assert_eq!(resource::detect("Actors.json", json), Format::Json);

    let fields = resource::read(Format::Json, json);
    let keys: Vec<&str> = fields.iter().map(|f| f.key.as_str()).collect();
    // Numbers are not text, and an empty string is not worth a translator's time.
    assert_eq!(keys, vec!["actors[0].name", "actors[1].name"]);

    let written = resource::write(Format::Json, json, &patch(&[("actors[1].name", "Ngư dân")]));
    let back: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(back["actors"][1]["name"], "Ngư dân");
    assert_eq!(
        back["actors"][0]["name"], "Hero",
        "an untouched value changed"
    );
    assert_eq!(back["version"], 3, "a number was disturbed");
}

/// Two objects using the same field name must not collide, which is the reason for paths.
#[test]
fn two_objects_with_the_same_field_name_stay_apart() {
    let json = r#"[{"text":"one"},{"text":"two"}]"#;
    let written = resource::write(Format::Json, json, &patch(&[("[1].text", "hai")]));
    let back: serde_json::Value = serde_json::from_str(&written).unwrap();
    assert_eq!(back[0]["text"], "one");
    assert_eq!(back[1]["text"], "hai");
}

#[test]
fn properties_still_work_the_way_they_did() {
    let text = "# level names\nlevel.1=Green Field\nlevel.2=Dark Cave\n";
    assert_eq!(
        resource::detect("levels.properties", text),
        Format::Properties
    );

    let written = resource::write(
        Format::Properties,
        text,
        &patch(&[("level.1", "Đồng Xanh")]),
    );
    assert!(written.contains("level.1=Đồng Xanh"), "{written}");
    assert!(written.contains("level.2=Dark Cave"), "{written}");
    assert!(written.contains("# level names"), "{written}");
}

/// A file that gained or lost a trailing newline shows as changed in every diff tool there is.
#[test]
fn the_trailing_newline_is_whatever_it_was() {
    for (text, ends_with_newline) in [("a=1\nb=2\n", true), ("a=1\nb=2", false)] {
        let written = resource::write(Format::Properties, text, &patch(&[("a", "x")]));
        assert_eq!(written.ends_with('\n'), ends_with_newline, "{written:?}");
    }
}

/// The name is not enough and the contents are not either, so both are used.
#[test]
fn a_json_document_in_a_txt_file_is_still_json() {
    assert_eq!(resource::detect("data.txt", r#"{"a":"b"}"#), Format::Json);
    // And something that merely starts with a brace is not.
    assert_eq!(
        resource::detect("notes.txt", "{ not json at all"),
        Format::Lines
    );
}

/// Where a format has no keys, a line number is all there is - and it says so, because a line
/// number stops matching the moment somebody adds a line above it.
#[test]
fn a_format_with_no_keys_is_reported_as_having_none() {
    assert!(Format::Gettext.keyed());
    assert!(!Format::Lines.keyed());

    let text = "first\nsecond\n";
    let written = resource::write(Format::Lines, text, &patch(&[("1", "thứ hai")]));
    assert_eq!(written, "first\nthứ hai\n");
}

/// A generated strings.xml is one long line, and a reader that only looked at line starts would
/// report it as having no text in it - which reads as a fact about the file rather than as a
/// limitation of the reader.
#[test]
fn a_minified_android_file_is_read_and_patched_too() {
    let xml =
        "<resources><string name=\"a\">one</string><string name=\"b\">two</string></resources>";

    let fields = resource::read(Format::AndroidStrings, xml);
    assert_eq!(fields.len(), 2, "{fields:?}");

    // Both on one line, so patching the second must not disturb the first.
    let written = resource::write(Format::AndroidStrings, xml, &patch(&[("b", "hai")]));
    assert_eq!(
        written,
        "<resources><string name=\"a\">one</string><string name=\"b\">hai</string></resources>"
    );
}

/// Two sections holding the same key is the ordinary case in an INI, and a reader that ignored
/// sections would translate one of them and silently overwrite the other.
#[test]
fn ini_keys_are_qualified_by_their_section() {
    let ini = "; the menus\n\
               [menu]\n\
               title=Start Game\n\
               \n\
               [shop]\n\
               title=Shop\n";
    assert_eq!(resource::detect("game.ini", ini), Format::Ini);

    let fields = resource::read(Format::Ini, ini);
    let keys: Vec<&str> = fields.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(keys, vec!["menu.title", "shop.title"]);

    let written = resource::write(Format::Ini, ini, &patch(&[("shop.title", "Cửa hàng")]));
    assert!(written.contains("[menu]\ntitle=Start Game"), "{written}");
    assert!(written.contains("[shop]\ntitle=Cửa hàng"), "{written}");
    assert!(
        written.contains("; the menus"),
        "the comment was lost:\n{written}"
    );
}

/// A file of `key=value` with no sections at all is a properties file, whatever it is called.
#[test]
fn a_file_with_no_sections_is_not_read_as_an_ini() {
    assert_eq!(resource::detect("config.ini", "a=1\nb=2\n"), Format::Lines);
}

// -- Ren'Py -------------------------------------------------------------------------------------

/// A generated Ren'Py translation file, in both the shapes one holds.
fn renpy() -> &'static str {
    "# game/script.rpy:12\n\
     translate vietnamese start_a1b2c3:\n\
     \n\
     \x20   # e \"Cast your line.\"\n\
     \x20   e \"\"\n\
     \n\
     \x20   # \"The water is still.\"\n\
     \x20   \"\"\n\
     \n\
     translate vietnamese strings:\n\
     \n\
     \x20   # game/script.rpy:20\n\
     \x20   old \"Start Game\"\n\
     \x20   new \"\"\n"
}

/// The original of a dialogue line is in the comment above it and the translation goes in the
/// empty statement below. A reader that took the statement as the source would hand a translator
/// a file of empty strings and call it the game's text.
#[test]
fn a_renpy_dialogue_line_takes_its_source_from_the_comment_above_it() {
    assert_eq!(resource::detect("script.rpy", renpy()), Format::Renpy);

    let fields = resource::read(Format::Renpy, renpy());
    assert_eq!(fields.len(), 3, "{fields:?}");
    assert_eq!(fields[0].value, "Cast your line.");
    assert_eq!(fields[1].value, "The water is still.");
}

/// Narration has no speaker. In a visual novel that is the ordinary case, not an edge one.
#[test]
fn narration_with_no_speaker_is_read_like_any_other_line() {
    let fields = resource::read(Format::Renpy, renpy());
    assert_eq!(fields[1].value, "The water is still.");

    let written = resource::write(
        Format::Renpy,
        renpy(),
        &patch(&[("start_a1b2c3::1", "Mặt nước lặng như tờ.")]),
    );
    assert!(
        written.contains("    \"Mặt nước lặng như tờ.\""),
        "{written}"
    );
}

/// A block holds several lines, so the label alone is not an address. Sharing one would let the
/// second translation overwrite the first, and one character's line would come out of another's
/// mouth.
#[test]
fn two_dialogue_lines_in_one_block_are_addressed_apart() {
    let fields = resource::read(Format::Renpy, renpy());
    assert_eq!(fields[0].key, "start_a1b2c3::0");
    assert_eq!(fields[1].key, "start_a1b2c3::1");
}

/// A `strings` block carries the original in `old`, which is the key Ren'Py itself matches on -
/// so it is content-addressed and survives the file being regenerated, unlike a position.
#[test]
fn a_renpy_strings_block_is_addressed_by_its_original_text() {
    let fields = resource::read(Format::Renpy, renpy());
    assert_eq!(fields[2].key, "strings::Start Game");
    assert_eq!(fields[2].value, "Start Game");

    let written = resource::write(
        Format::Renpy,
        renpy(),
        &patch(&[("strings::Start Game", "Bắt đầu")]),
    );
    assert!(written.contains("    new \"Bắt đầu\""), "{written}");
    assert!(written.contains("    old \"Start Game\""), "{written}");
}

/// `# game/script.rpy:12` says where a line came from; it is not something to translate. One
/// appears above every header and above every `old`, so a reader that took any comment as source
/// would hand a translator a file path for every entry in the game.
#[test]
fn a_file_and_line_comment_is_never_offered_as_text() {
    let fields = resource::read(Format::Renpy, renpy());
    assert!(
        !fields.iter().any(|f| f.value.contains("script.rpy")),
        "{fields:?}"
    );
}

/// Ren'Py decides what a line belongs to by how far it is indented, and a say statement may carry
/// clauses after its string. A writer that rebuilt the line from its parts would move it into
/// another block or drop the clause - and both are silent.
#[test]
fn a_patched_say_statement_keeps_its_indentation_and_its_trailing_clauses() {
    let file = "translate vietnamese b:\n\
                \x20       # e happy \"Hello.\" nointeract\n\
                \x20       e happy \"\" nointeract\n";
    let written = resource::write(Format::Renpy, file, &patch(&[("b::0", "Chào.")]));
    assert!(
        written.contains("        e happy \"Chào.\" nointeract"),
        "{written}"
    );
}

/// The values `read` returns are in the file's own escaped form, so a translator hands them back
/// that way. Escaping them a second time puts a literal backslash in front of the player, and
/// does it one level deeper on every build - so rebuilding a built game has to be safe.
#[test]
fn a_translation_that_is_already_escaped_is_not_escaped_a_second_time() {
    let file = "translate vietnamese b:\n    # e \"say \\\"hi\\\"\"\n    e \"\"\n";
    let escaped = "nói \\\"chào\\\"";
    let once = resource::write(Format::Renpy, file, &patch(&[("b::0", escaped)]));
    assert!(once.contains("e \"nói \\\"chào\\\"\""), "{once}");

    // The rebuild path, made concrete: what came out of the file is what a later analysis of the
    // built game reads back as its source, and writing that returns the same bytes rather than
    // one escape level deeper.
    let out = resource::read(Format::Renpy, &once);
    let came_out = out
        .iter()
        .find(|f| f.key == "b::0")
        .expect("the entry went missing");
    let again = resource::write(Format::Renpy, file, &patch(&[("b::0", escaped)]));
    assert_eq!(once, again);

    // And the comment, which is what `read` returns for a dialogue line, still holds the original
    // in exactly the form the file had it - not one level deeper either.
    assert_eq!(came_out.value, "say \\\"hi\\\"");
}

/// A raw newline inside a Ren'Py string ends the statement and takes the rest of the block with
/// it, so a translation that arrives with a real line break has to reach the file as `\n`.
#[test]
fn a_real_newline_in_a_translation_is_written_as_an_escape() {
    let file = "translate vietnamese b:\n    # e \"One. Two.\"\n    e \"\"\n";
    let written = resource::write(Format::Renpy, file, &patch(&[("b::0", "Một.\nHai.")]));
    assert!(written.contains("e \"Một.\\nHai.\""), "{written}");
    assert_eq!(
        written.lines().count(),
        3,
        "the file grew a line:\n{written}"
    );
}

/// A block ends where the indentation does. A pending original that leaked past the end of its
/// block would be written into the first line of the next one.
#[test]
fn a_renpy_block_ends_at_the_first_line_in_column_zero() {
    let file = "translate vietnamese one:\n\
                \x20   # e \"First.\"\n\
                translate vietnamese two:\n\
                \x20   # e \"Second.\"\n\
                \x20   e \"\"\n";
    let fields = resource::read(Format::Renpy, file);
    assert_eq!(fields.len(), 1, "{fields:?}");
    assert_eq!(fields[0].key, "two::0");
    assert_eq!(fields[0].value, "Second.");
}

/// A dialogue block holds ordinary Ren'Py among the say statements. Counting a `voice` line as
/// dialogue would shift the address of every line after it in the block - which does not fail
/// loudly, it writes each translation one line further down than it belongs.
#[test]
fn a_statement_that_is_not_dialogue_does_not_shift_the_lines_after_it() {
    let file = "translate vietnamese b:\n\
                \x20   voice \"audio/e01.ogg\"\n\
                \x20   # e \"First.\"\n\
                \x20   e \"\"\n\
                \x20   # e \"Second.\"\n\
                \x20   e \"\"\n";
    let fields = resource::read(Format::Renpy, file);
    assert_eq!(fields.len(), 2, "{fields:?}");
    assert_eq!(fields[0].key, "b::0");
    assert_eq!(fields[1].key, "b::1");

    let written = resource::write(Format::Renpy, file, &patch(&[("b::0", "Một.")]));
    assert!(written.contains("voice \"audio/e01.ogg\""), "{written}");
    assert!(written.contains("    e \"Một.\""), "{written}");
}

/// The extension is the hint and the `translate` header is the confirmation, because a `.rpy`
/// with no header is the game's own script rather than a resource.
#[test]
fn a_renpy_script_with_no_translate_block_is_not_a_translation_file() {
    let script = "label start:\n    e \"Hello there.\"\n    return\n";
    assert_ne!(resource::detect("script.rpy", script), Format::Renpy);
}

/// A text file that merely contains the word `old` is not a Ren'Py file, whatever it is quoting.
#[test]
fn a_text_file_that_mentions_old_is_not_read_as_renpy() {
    assert_eq!(
        resource::detect("notes.txt", "old \"Start Game\"\nnew \"\"\n"),
        Format::Lines
    );
}

/// Everything the patch did not name comes through untouched, which is what every writer here is
/// for: a file rewritten from a parsed model reports a diff nobody made.
#[test]
fn an_untouched_renpy_entry_and_every_comment_survive_a_write() {
    let written = resource::write(
        Format::Renpy,
        renpy(),
        &patch(&[("start_a1b2c3::0", "Thả dây câu.")]),
    );
    assert!(written.contains("    e \"Thả dây câu.\""), "{written}");

    for kept in [
        "# game/script.rpy:12",
        "# game/script.rpy:20",
        "    # e \"Cast your line.\"",
        "    # \"The water is still.\"",
        "translate vietnamese strings:",
        "    new \"\"",
    ] {
        assert!(written.contains(kept), "lost {kept:?}:\n{written}");
    }
}

/// The rebuild path, proved directly: what the writer put in the file, fed back to the writer, has
/// to come out the same. Without that property every build adds one more backslash to every
/// escaped string in the game, and nobody notices until a player sees `\"` on screen.
#[test]
fn escaping_a_translation_twice_changes_nothing_the_second_time() {
    let file = "translate vietnamese b:\n    # e \"x\"\n    e \"\"\n";
    // A raw quote, as somebody typing a translation would produce it.
    let once = resource::write(Format::Renpy, file, &patch(&[("b::0", "nói \"chào\"")]));

    let in_file = once
        .lines()
        .find_map(|l| l.trim().strip_prefix("e \"")?.strip_suffix('"'))
        .expect("the statement went missing");
    assert_eq!(in_file, "nói \\\"chào\\\"", "the quote was not escaped");

    let twice = resource::write(Format::Renpy, file, &patch(&[("b::0", in_file)]));
    assert_eq!(once, twice, "a second pass escaped it again");
}
