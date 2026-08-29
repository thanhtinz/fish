//! Post-build validation (specification §24).
//!
//! Checks run against the built artefact, not against the plan, because the failures that matter
//! are the ones that survive the build: a class that no longer parses, a MIDlet entry point that
//! disappeared, a placeholder lost in translation.

use crate::classfile::ClassFile;
use crate::graph::{ContentGraph, ContextType};
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

    /// Adds findings produced by a check that needs more than the built archive to run.
    ///
    /// Some checks need the project directory - whether a redrawn image exists on disk, say - and
    /// threading the project through every signature here would put a file system behind a module
    /// whose whole point is that it only looks at the artefact. So those checks are functions that
    /// return findings, and this is where they land.
    pub fn extend(&mut self, findings: impl IntoIterator<Item = Finding>) {
        self.findings.extend(findings);
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

/// Everything a full validation looks at.
///
/// A struct rather than nine arguments, and it became one when the ninth was needed: the checks
/// have to know what kind of package this is. Half of them are rules about JAR files - a MIDlet
/// entry point, a glyph sheet - and running those against an Android package produces errors
/// about a format it is not in, which is how a person learns to ignore the report.
pub struct Subject<'a> {
    pub original: &'a Archive,
    pub built: &'a Archive,
    pub graph: &'a ContentGraph,
    pub translations: &'a TranslationStore,
    pub from: &'a Language,
    pub to: &'a Language,
    pub kind: crate::package::Kind,
    /// What the game's font can draw, where anybody has established it.
    pub font: Option<&'a crate::font::Coverage>,
    /// How wide its letters are, where it draws from a sheet.
    pub metrics: Option<&'a crate::font::metrics::Metrics>,
}

impl<'a> Subject<'a> {
    /// The minimum: two archives, a graph, and the languages involved.
    ///
    /// Assumes a MIDlet, because that is what a caller with nothing else to say has - and the
    /// checks it turns on are the ones this project started with.
    pub fn new(
        original: &'a Archive,
        built: &'a Archive,
        graph: &'a ContentGraph,
        translations: &'a TranslationStore,
        from: &'a Language,
        to: &'a Language,
    ) -> Self {
        Subject {
            original,
            built,
            graph,
            translations,
            from,
            to,
            kind: crate::package::Kind::Midlet,
            font: None,
            metrics: None,
        }
    }
}

/// Validates a built archive against the original it came from.
///
/// The languages are needed because the translation checks are language-specific: what counts as
/// too long, or as wrongly spaced, has no answer that holds for Vietnamese and Chinese at once.
pub fn validate(subject: &Subject) -> ValidationReport {
    let mut report = ValidationReport::default();

    check_nothing_lost(subject.original, subject.built, &mut report);
    check_classes_parse(subject.built, &mut report);
    check_translations(
        subject.graph,
        subject.translations,
        subject.from,
        subject.to,
        &mut report,
    );
    check_layout(
        subject.metrics,
        subject.graph,
        subject.translations,
        &mut report,
    );
    check_originals_preserved(subject.original, subject.built, &mut report);

    // Rules about the JAR format, asked only of JAR files. An Android package has no MIDlet entry
    // point and draws with the platform's fonts; reporting both as missing would be reporting
    // that it is not a J2ME game, which nobody needed telling.
    if matches!(
        subject.kind,
        crate::package::Kind::Midlet | crate::package::Kind::JavaArchive
    ) {
        check_entry_points(subject.built, &mut report);
        check_font(
            subject.font,
            subject.graph,
            subject.translations,
            &mut report,
        );
    } else if subject.font.is_some() {
        // Unless somebody did establish a font for it, in which case they know something this
        // does not and the check is theirs to have asked for.
        check_font(
            subject.font,
            subject.graph,
            subject.translations,
            &mut report,
        );
    }

    report
}

/// Interface text that will not fit where the original fitted.
///
/// The check nobody can do from a character count, and the one that matters most for Vietnamese:
/// a translation gains letters and diacritics, and a button sized for "Exit" was not sized for
/// "Thoát trò chơi".
///
/// Three deliberate limits, because the alternative is a check people learn to ignore:
///
/// - **Only interface text.** Dialogue and story wrap; a long line there is a line, not a bug.
/// - **Only proportional sheets.** Where every letter is the width of its cell, this measurement
///   is the character count in different units, and `check_translations` already made that point.
/// - **A warning, never an error.** Nothing here knows how wide the button is. What it knows is
///   that the original fitted, so a translation much wider than it is a risk - which is a weaker
///   claim than "this overflows", and the one the data supports.
///
/// The threshold is its own number rather than the language's `expansion_limit`. That one is a
/// character-count heuristic, set loose (three times) because character counts across scripts are
/// a blunt instrument. Pixels are not blunt: a label half again as wide as the one the layout was
/// drawn for is past what ordinary padding absorbs, and a limit of three would let almost
/// everything through and make this check decoration.
const WIDTH_LIMIT: f32 = 1.5;
fn check_layout(
    metrics: Option<&crate::font::metrics::Metrics>,
    graph: &ContentGraph,
    translations: &TranslationStore,
    report: &mut ValidationReport,
) {
    let Some(metrics) = metrics else { return };
    if metrics.monospaced {
        return;
    }

    for node in &graph.nodes {
        if node.context != ContextType::Ui {
            continue;
        }
        let Some(target) = translations.get(&node.id) else {
            continue;
        };
        // A string the sheet cannot draw has a bigger problem, and `check_font` reports it. A
        // second complaint about the same string, in pixels invented for glyphs that are not
        // there, would only bury the first.
        let (Some(before), Some(after)) =
            (metrics.measure(&node.source_text), metrics.measure(target))
        else {
            continue;
        };
        if before == 0 {
            continue;
        }

        let grown = after as f32 / before as f32;
        // The few pixels a short label gains are not what overflows a screen, and flagging them
        // would bury the cases that do.
        if grown > WIDTH_LIMIT && after.saturating_sub(before) >= metrics.cell_width {
            report.warn(
                "layout.width",
                format!(
                    "{target:?} draws {after} pixels wide against {before} for {:?} - it may not \
                     fit where the original did",
                    node.source_text
                ),
            );
        }
    }
}

/// Images somebody marked as carrying words, and whether anything was done about them (§17).
///
/// The whole reason for recording them: a translation can be complete, correct and validated, and
/// still show a player an English START button, because the word was painted into the artwork
/// rather than stored as a string. Nothing in this project can read those words. What it can do
/// is refuse to let them be forgotten.
///
/// Warnings, not errors. Shipping with the original artwork is a normal decision - redrawing a
/// logo is real work, and sometimes the answer is "not this release". An error would make the
/// build refuse over something a person already decided.
pub fn check_text_assets(
    assets: &[crate::assets::TextAsset],
    root: &std::path::Path,
    built: &Archive,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for asset in assets {
        let says = if asset.says.is_empty() {
            String::new()
        } else {
            format!(" (it says {:?})", asset.says)
        };
        let Some(replacement) = &asset.replacement else {
            findings.push(Finding {
                severity: Severity::Warning,
                check: "asset.text".into(),
                detail: format!(
                    "{} has words painted into it{says} and nothing to replace it - the build will ship them untranslated",
                    asset.entry
                ),
            });
            continue;
        };

        let path = root.join(replacement);
        let Ok(wanted) = std::fs::read(&path) else {
            findings.push(Finding {
                severity: Severity::Warning,
                check: "asset.text".into(),
                detail: format!(
                    "{} names {replacement} as its replacement, and that file is not there",
                    asset.entry
                ),
            });
            continue;
        };

        // Having the redrawn file is not the same as shipping it. Installing an image is a rule
        // (§19), and a rule that was written but never switched on leaves the artwork untouched
        // while everything on screen says the work was done.
        match built.get(&asset.entry) {
            Some(shipped) if shipped.data == wanted => {}
            Some(_) => findings.push(Finding {
                severity: Severity::Warning,
                check: "asset.text".into(),
                detail: format!(
                    "{} was redrawn as {replacement}, but the build still carries the original - a rule has to install it",
                    asset.entry
                ),
            }),
            None => findings.push(Finding {
                severity: Severity::Warning,
                check: "asset.text".into(),
                detail: format!("{} is marked as carrying text but is not in the build", asset.entry),
            }),
        }
    }
    findings
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
