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
    pub output_sha256: String,
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
    let mut archive = Archive::read(&original.write()?)?;
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
        // Resources are rewritten as UTF-8 regardless of what they were read as: the game reads
        // them through its own loader, and any charset that could not represent Vietnamese is the
        // reason the text needed localizing in the first place.
        let text = String::from_utf8_lossy(&entry.data).into_owned();
        let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();

        for (source, target) in patches {
            match source {
                TextSource::ResourceProperty { key, .. } => {
                    for line in lines.iter_mut() {
                        if let Some((k, _)) = line.split_once('=') {
                            if k.trim() == key {
                                *line = format!("{k}={target}");
                                report.resources_patched += 1;
                                break;
                            }
                        }
                    }
                }
                TextSource::ResourceLine { line, .. } => {
                    if let Some(slot) = lines.get_mut(*line) {
                        *slot = (*target).to_string();
                        report.resources_patched += 1;
                    }
                }
                TextSource::ClassConstant { .. } => unreachable!(),
            }
        }
        let mut rebuilt = lines.join("\n");
        rebuilt.push('\n');
        archive.replace(resource_name, rebuilt.into_bytes());
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
