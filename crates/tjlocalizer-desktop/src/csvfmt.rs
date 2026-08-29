//! Reading and writing the CSV a translator works in outside this application.
//!
//! Written out rather than pulled in, and tested, because the failure mode is silent: game text is
//! full of commas, quotes and newlines, and a field that lands in the wrong column is approved as
//! the translation of the wrong string. Nobody notices until the game ships.

/// Quotes a field. Doubling an embedded quote is the format's own escape.
pub fn quote(field: &str) -> String {
    format!("\"{}\"", field.replace('"', "\"\""))
}

/// Splits one CSV line, honouring quotes and doubled quotes.
pub fn parse_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => fields.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields
}

/// A UTF-8 byte-order mark.
///
/// Excel reads a CSV without one as the system's legacy encoding, which turns every Vietnamese
/// diacritic and every Thai character into rubbish - and the translator's tool is Excel.
pub const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
