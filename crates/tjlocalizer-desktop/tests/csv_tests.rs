//! The CSV a translator works in. Silent corruption here approves the wrong string.

use tjlocalizer_desktop_lib::csvfmt::{parse_line, quote, BOM};

fn round_trip(fields: &[&str]) -> Vec<String> {
    let line = fields
        .iter()
        .map(|f| quote(f))
        .collect::<Vec<_>>()
        .join(",");
    parse_line(&line)
}

#[test]
fn plain_fields_survive() {
    assert_eq!(
        round_trip(&["a1", "ui", "Main.class", "Quit", "Thoát"]),
        vec!["a1", "ui", "Main.class", "Quit", "Thoát"]
    );
}

/// The one that matters. Game text is full of commas, and a field split on one lands in the next
/// column - so the translation of one string is approved as the translation of another.
#[test]
fn a_comma_inside_a_translation_does_not_split_the_row() {
    let fields = round_trip(&[
        "id",
        "dialogue",
        "Main.class",
        "You have arrived at last, traveller.",
        "Rốt cuộc ngươi cũng tới rồi, lữ khách.",
    ]);
    assert_eq!(fields.len(), 5);
    assert_eq!(fields[3], "You have arrived at last, traveller.");
    assert_eq!(fields[4], "Rốt cuộc ngươi cũng tới rồi, lữ khách.");
}

#[test]
fn quotes_inside_a_field_survive() {
    let fields = round_trip(&["id", "ui", "Main.class", "Press \"OK\"", "Bấm \"Đồng ý\""]);
    assert_eq!(fields[3], "Press \"OK\"");
    assert_eq!(fields[4], "Bấm \"Đồng ý\"");
}

#[test]
fn an_empty_translation_stays_an_empty_field_rather_than_disappearing() {
    let fields = round_trip(&["id", "ui", "Main.class", "Quit", ""]);
    assert_eq!(fields.len(), 5);
    assert_eq!(fields[4], "");
}

#[test]
fn a_field_that_is_only_punctuation_survives() {
    let fields = round_trip(&["id", "format", "Main.class", "%d / %d", "%d / %d"]);
    assert_eq!(fields[3], "%d / %d");
    assert_eq!(fields[4], "%d / %d");
}

/// Non-Latin scripts must come back exactly, or the export is useless for the languages this
/// tool exists to produce.
#[test]
fn every_target_script_round_trips() {
    for text in ["装备强化", "การตั้งค่า", "설정", "Настройки", "Trang bị"]
    {
        let fields = round_trip(&["id", "ui", "Main.class", "Settings", text]);
        assert_eq!(fields[4], text);
    }
}

#[test]
fn the_bom_is_what_excel_needs_to_read_utf8() {
    assert_eq!(BOM, [0xEF, 0xBB, 0xBF]);
}
