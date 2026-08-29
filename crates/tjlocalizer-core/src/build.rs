//! Applying approved translations and repackaging (specification §23, §26).

use crate::classfile::ClassFile;
use crate::error::Result;
use crate::graph::{ContentGraph, TextSource};
use crate::jar::{Archive, Manifest};
use crate::translation::TranslationStore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Who did the localization, for the attribution written into the output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branding {
    pub enabled: bool,
    pub author: String,
    pub localization_version: String,
    pub year: String,
}

impl Default for Branding {
    fn default() -> Self {
        Self {
            enabled: true,
            author: "Thanhtinz".to_string(),
            localization_version: "1.0.0".to_string(),
            year: "2026".to_string(),
        }
    }
}

/// What a build did.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildReport {
    pub classes_patched: usize,
    pub literals_patched: usize,
    pub resources_patched: usize,
    pub skipped: Vec<String>,
    /// Resources this build can read but not write, that had translations waiting for them.
    ///
    /// Structured rather than folded into `skipped`, because the interface shows these and the
    /// count matters: a translator who approved four hundred lines needs to know they are not in
    /// the file, and needs it as one fact rather than four hundred.
    #[serde(default)]
    pub refused: Vec<Refusal>,
    pub output_sha256: String,
}

/// A resource that was left alone, and what was waiting for it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Refusal {
    pub resource: String,
    pub reason: String,
    /// How many approved translations will not appear because of this.
    pub translations: usize,
}

/// Applies every approved translation and returns the rebuilt archive.
///
/// Patches are grouped by class so each one is parsed and re-serialised once, however many of its
/// literals changed.
pub fn apply(
    original: &Archive,
    graph: &ContentGraph,
    translations: &TranslationStore,
    branding: &Branding,
) -> Result<(Archive, BuildReport)> {
    apply_with(
        original,
        graph,
        translations,
        branding,
        &crate::plugin::Formats::default(),
    )
}

/// The same, where plugins have named files this build would not have recognised (§20).
///
/// The extractor and the build have to be told the same thing: a plugin that made a file
/// translatable at extraction and not at build time would collect translations nobody could
/// ship, which is the failure this crate spends `writeback` avoiding.
pub fn apply_with(
    original: &Archive,
    graph: &ContentGraph,
    translations: &TranslationStore,
    branding: &Branding,
    formats: &crate::plugin::Formats,
) -> Result<(Archive, BuildReport)> {
    // A plain clone. This used to zip the whole archive and parse it straight back, which for a
    // game directory would mean compressing forty thousand files in order to change three strings.
    let mut archive = original.clone();
    let mut report = BuildReport::default();

    let mut by_class: BTreeMap<&str, Vec<(u16, &str)>> = BTreeMap::new();
    let mut by_resource: BTreeMap<&str, Vec<(&TextSource, &str)>> = BTreeMap::new();

    for node in &graph.nodes {
        let Some(target) = translations.get(&node.id) else {
            continue;
        };
        if !node.context.is_translatable() {
            // Belt and braces: a technical string should never have been approved, but applying
            // one would break the game rather than merely read badly.
            report
                .skipped
                .push(format!("{}: not translatable", node.source_text));
            continue;
        }
        match &node.source {
            TextSource::ClassConstant {
                class, utf8_index, ..
            } => {
                by_class
                    .entry(class.as_str())
                    .or_default()
                    .push((*utf8_index, target));
            }
            source => {
                let resource = match source {
                    TextSource::ResourceProperty { resource, .. } => resource.as_str(),
                    TextSource::ResourceLine { resource, .. } => resource.as_str(),
                    TextSource::ClassConstant { .. } => unreachable!(),
                };
                by_resource
                    .entry(resource)
                    .or_default()
                    .push((source, target));
            }
        }
    }

    for (class_name, patches) in by_class {
        let Some(entry) = archive.get(class_name) else {
            report.skipped.push(format!("{class_name}: not in archive"));
            continue;
        };
        let mut class = ClassFile::parse(&entry.data)?;
        for (index, text) in &patches {
            class.set_utf8_text(*index, text)?;
            report.literals_patched += 1;
        }
        archive.replace(class_name, class.write()?);
        report.classes_patched += 1;
    }

    for (resource_name, patches) in by_resource {
        let Some(entry) = archive.get(resource_name) else {
            report
                .skipped
                .push(format!("{resource_name}: not in archive"));
            continue;
        };
        // One question, asked in one place. Before this, the fallback below decoded every patched
        // resource with `from_utf8_lossy` and wrote it back - which for a binary file means every
        // invalid byte becomes U+FFFD and the file is destroyed while the build reports success.
        match crate::writeback::plan_with(resource_name, &entry.data, formats) {
            crate::writeback::Plan::ReadOnly { reason } => {
                report.refused.push(Refusal {
                    resource: resource_name.to_string(),
                    reason,
                    translations: patches.len(),
                });
                continue;
            }

            crate::writeback::Plan::Binary(crate::writeback::BinaryFormat::Locres) => {
                let mut table = crate::locres::Locres::parse(&entry.data)?;
                for (source, target) in &patches {
                    let TextSource::ResourceProperty { key, .. } = source else {
                        continue;
                    };
                    if table.set_at(key, target) {
                        report.resources_patched += 1;
                    } else {
                        // An entry that has moved between versions of the game is not an error
                        // worth stopping for, but it is a translation that will not appear - and
                        // silence there reads as success.
                        report
                            .skipped
                            .push(format!("{resource_name}: no entry {key}"));
                    }
                }
                archive.replace(resource_name, table.write());
            }

            crate::writeback::Plan::Text { format, .. } => {
                // Rewritten as UTF-8 regardless of what it was read as: the game reads it through
                // its own loader, and any charset that could not represent Vietnamese is the
                // reason the text needed localizing in the first place.
                let text = String::from_utf8_lossy(&entry.data).into_owned();

                let mut wanted: BTreeMap<String, String> = BTreeMap::new();
                for (source, target) in patches {
                    let key = match source {
                        TextSource::ResourceProperty { key, .. } => key.clone(),
                        TextSource::ResourceLine { line, .. } => line.to_string(),
                        TextSource::ClassConstant { .. } => unreachable!(),
                    };
                    wanted.insert(key, (*target).to_string());
                }
                report.resources_patched += wanted.len();
                let rebuilt = crate::resource::write(format, &text, &wanted);
                archive.replace(resource_name, rebuilt.into_bytes());
            }
        }
    }

    if branding.enabled {
        apply_branding(&mut archive, branding);
    }

    report.output_sha256 = crate::jar::sha256_hex(&archive.write()?);
    Ok((archive, report))
}

/// Writes attribution for the localization without claiming the game.
///
/// The distinction the specification draws in §36 is load-bearing and is implemented here rather
/// than left to documentation: original manifest attributes are never removed or rewritten, and
/// the attribution added is explicitly scoped to the localization.
fn apply_branding(archive: &mut Archive, branding: &Branding) {
    let notice = format!(
        "Vietnamese Localization: {author}\n\
         Localization Version: {version}\n\
         Localization Copyright: (c) {year} {author}. All rights reserved.\n\
         \n\
         This file records authorship of the Vietnamese localization only.\n\
         Rights in the original application remain with its owner, and the\n\
         original manifest and notices are preserved unchanged.\n",
        author = branding.author,
        version = branding.localization_version,
        year = branding.year,
    );
    archive.insert("META-INF/THANHTINZ.BRAND", notice.into_bytes());

    let mut localization = Manifest::default();
    localization.set("Manifest-Version", "1.0");
    localization.set("Localization-Author", &branding.author);
    localization.set("Localization-Version", &branding.localization_version);
    localization.set("Localization-Language", "vi-VN");
    localization.set("Localization-Tool", "Thanhtinz JAR Localizer");
    archive.insert(
        "META-INF/LOCALIZATION.MF",
        localization.render().into_bytes(),
    );
}
