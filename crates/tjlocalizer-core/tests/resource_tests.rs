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
