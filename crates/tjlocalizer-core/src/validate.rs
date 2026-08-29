//! Post-build validation (specification §24).
//!
//! Checks run against the built artefact, not against the plan, because the failures that matter
//! are the ones that survive the build: a class that no longer parses, a MIDlet entry point that
//! disappeared, a placeholder lost in translation.

use crate::classfile::ClassFile;
use crate::graph::ContentGraph;
use crate::jar::{Archive, Manifest};
use crate::lang::Language;
use crate::translation::TranslationStore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The output is broken and must not ship.
    Error,
    /// Worth a look, but the output is usable.
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub check: String,
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationReport {
    pub findings: Vec<Finding>,
}

impl ValidationReport {
    pub fn is_ok(&self) -> bool {
        !self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
    }

    fn error(&mut self, check: &str, detail: String) {
        self.findings.push(Finding {
            severity: Severity::Error,
            check: check.to_string(),
            detail,
        });
    }

    fn warn(&mut self, check: &str, detail: String) {
        self.findings.push(Finding {
            severity: Severity::Warning,
            check: check.to_string(),
            detail,
        });
    }
}

/// Validates a built archive against the original it came from.
///
/// The languages are needed because the translation checks are language-specific: what counts as
/// too long, or as wrongly spaced, has no answer that holds for Vietnamese and Chinese at once.
pub fn validate(
    original: &Archive,
    built: &Archive,
    graph: &ContentGraph,
    translations: &TranslationStore,
    from: &Language,
    to: &Language,
) -> ValidationReport {
    validate_with_font(original, built, graph, translations, from, to, None)
}

/// Validation including the glyph check, when the game's font has been established.
#[allow(clippy::too_many_arguments)]
pub fn validate_with_font(
    original: &Archive,
    built: &Archive,
    graph: &ContentGraph,
    translations: &TranslationStore,
    from: &Language,
    to: &Language,
    font: Option<&crate::font::Coverage>,
) -> ValidationReport {
    let mut report = ValidationReport::default();

    check_nothing_lost(original, built, &mut report);
    check_classes_parse(built, &mut report);
    check_entry_points(built, &mut report);
    check_translations(graph, translations, from, to, &mut report);
    check_font(font, graph, translations, &mut report);
    check_originals_preserved(original, built, &mut report);

    report
}

/// Validates an archive on its own, with no original to compare against.
///
/// This is what can be said about a JAR handed over without its project: it is well formed, every
/// class parses, it declares an entry point that exists, and its text decodes. It cannot tell
/// whether anything was lost relative to the original - only `validate` can - so the two are kept
/// separate rather than one function pretending to do both.
pub fn inspect(archive: &Archive) -> ValidationReport {
    let mut report = ValidationReport::default();
    check_classes_parse(archive, &mut report);
    check_entry_points(archive, &mut report);
    check_class_text_decodes(archive, &mut report);
    report
}

/// Mojibake check: a constant that no longer decodes as modified UTF-8 means a patch wrote raw
/// bytes in some other encoding, which the JVM will reject or display as rubbish.
fn check_class_text_decodes(archive: &Archive, report: &mut ValidationReport) {
    for entry in archive.classes() {
        let Ok(class) = ClassFile::parse(&entry.data) else {
            continue; // already reported by check_classes_parse
        };
        for literal in class.string_literals() {
            if literal.decoded.is_none() {
                report.error(
                    "encoding",
                    format!(
                        "{}: string constant {} is not valid modified UTF-8",
                        entry.name, literal.utf8_index
                    ),
                );
            }
        }
    }
}

/// Every entry in the original must still be present.
fn check_nothing_lost(original: &Archive, built: &Archive, report: &mut ValidationReport) {
    for entry in original.entries() {
        if built.get(&entry.name).is_none() {
            report.error(
                "resource",
                format!("{} is missing from the build", entry.name),
            );
        }
    }
}

/// Every class must still parse. This is the check that catches a bad patch.
fn check_classes_parse(built: &Archive, report: &mut ValidationReport) {
    for entry in built.classes() {
        if let Err(e) = ClassFile::parse(&entry.data) {
            report.error("class", format!("{} no longer parses: {e}", entry.name));
        }
    }
}

/// The MIDlet classes named in the manifest must exist in the archive.
///
/// This is the most common way a repackaged J2ME game fails: it installs and then will not start,
/// with no diagnostic beyond a blank screen.
fn check_entry_points(built: &Archive, report: &mut ValidationReport) {
    let Some(entry) = built.get("META-INF/MANIFEST.MF") else {
        report.error("manifest", "META-INF/MANIFEST.MF is missing".into());
        return;
    };
    let manifest = Manifest::parse(&String::from_utf8_lossy(&entry.data));
    let midlets = manifest.midlet_classes();
    if midlets.is_empty() {
        report.warn("manifest", "no MIDlet entry point declared".into());
    }
    for class in midlets {
        let path = format!("{}.class", class.replace('.', "/"));
        if built.get(&path).is_none() {
            report.error(
                "entry_point",
                format!("MIDlet class {class} has no {path} in the archive"),
            );
        }
    }
}

/// Every approved translation must survive its own quality checks.
///
/// Placeholders are the ones that break a running game, so they are errors; the rest are
/// warnings, because a translator may have had a reason.
fn check_translations(
    graph: &ContentGraph,
    translations: &TranslationStore,
    from: &Language,
    to: &Language,
    report: &mut ValidationReport,
) {
    for node in &graph.nodes {
        let Some(target) = translations.get(&node.id) else {
            continue;
        };
        let mut issues = crate::quality::check(
            &node.source_text,
            target,
            &node.constraints.placeholders,
            from,
            to,
        );
        if to.base() == "vi" {
            issues.extend(crate::vietnamese::check(target));
        }
        for issue in issues {
            let severity = if issue.code == "placeholder" || issue.code == "empty" {
                Severity::Error
            } else {
                Severity::Warning
            };
            report.findings.push(Finding {
                severity,
                check: format!("translation.{}", issue.code),
                detail: format!("{:?}: {}", node.source_text, issue.detail),
            });
        }
    }
}

/// Text the game's font cannot draw (specification §16, §24).
///
/// A translation using a glyph the font does not have passes every other check there is: right
/// length, right script, right spacing, placeholders intact. It also shows the player a blank.
/// This is the only check that sees it, and it needs the font to have been established - which is
/// why a project with no font profile gets a warning rather than silence.
pub fn check_font(
    coverage: Option<&crate::font::Coverage>,
    graph: &ContentGraph,
    translations: &TranslationStore,
    report: &mut ValidationReport,
) {
    let Some(coverage) = coverage else {
        if !translations.is_empty() {
            report.warn(
                "font",
                "no font is established for this game, so nothing can say whether the translations will display; if it draws from a glyph sheet rather than the device font, they will not"
                    .into(),
            );
        }
        return;
    };

    for node in &graph.nodes {
        let Some(target) = translations.get(&node.id) else {
            continue;
        };
        let missing = coverage.missing_in(target);
        if missing.is_empty() {
            continue;
        }
        report.error(
            "font.glyph",
            format!(
                "{:?}: the font has no glyph for {} - this will show as blanks",
                target,
                missing.iter().collect::<String>()
            ),
        );
    }
}

/// The original manifest attributes must survive the build (specification §36).
fn check_originals_preserved(original: &Archive, built: &Archive, report: &mut ValidationReport) {
    let (Some(before), Some(after)) = (
        original.get("META-INF/MANIFEST.MF"),
        built.get("META-INF/MANIFEST.MF"),
    ) else {
        return;
    };
    let before = Manifest::parse(&String::from_utf8_lossy(&before.data));
    let after = Manifest::parse(&String::from_utf8_lossy(&after.data));

    for (key, value) in before.iter() {
        match after.get(key) {
            None => report.error(
                "attribution",
                format!("original manifest attribute {key} was removed"),
            ),
            Some(now) if now != value => report.error(
                "attribution",
                format!("original manifest attribute {key} was changed"),
            ),
            _ => {}
        }
    }
}
