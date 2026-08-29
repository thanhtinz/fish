//! Capability detection (specification §6).
//!
//! The engine never asks "which game is this?". It asks what the archive *contains* and emits a
//! capability manifest; rules and plugins then key off those capabilities. That is what keeps
//! game-specific knowledge out of the core - there is no place here where a game name could be
//! written even if someone wanted to.

use crate::classfile::ClassFile;
use crate::jar::{Archive, Manifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A detected capability, with the evidence that produced it.
///
/// Evidence is carried rather than discarded because detection is heuristic: when a rule fires on
/// a capability and the result is wrong, the only way to find out why is to see what convinced
/// the detector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub confidence: f32,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityManifest {
    pub capabilities: Vec<Capability>,
}

impl CapabilityManifest {
    pub fn has(&self, id: &str) -> bool {
        self.capabilities.iter().any(|c| c.id == id)
    }

    pub fn ids(&self) -> BTreeSet<&str> {
        self.capabilities.iter().map(|c| c.id.as_str()).collect()
    }

    fn add(&mut self, id: &str, confidence: f32, evidence: Vec<String>) {
        self.capabilities.push(Capability {
            id: id.to_string(),
            confidence,
            evidence,
        });
    }
}

/// Inspects an archive and reports what it can work with.
pub fn detect(archive: &Archive) -> CapabilityManifest {
    let mut manifest = CapabilityManifest::default();

    detect_platform(archive, &mut manifest);
    detect_text(archive, &mut manifest);
    detect_resources(archive, &mut manifest);
    detect_obfuscation(archive, &mut manifest);

    manifest
}

fn detect_platform(archive: &Archive, out: &mut CapabilityManifest) {
    let Some(entry) = archive.get("META-INF/MANIFEST.MF") else {
        return;
    };
    let manifest = Manifest::parse(&String::from_utf8_lossy(&entry.data));

    if let Some(config) = manifest.get("MicroEdition-Configuration") {
        let id = if config.contains("1.1") {
            "cldc11"
        } else {
            "cldc10"
        };
        out.add(
            id,
            1.0,
            vec![format!("MicroEdition-Configuration: {config}")],
        );
    }
    if let Some(profile) = manifest.get("MicroEdition-Profile") {
        let id = if profile.contains("2.") {
            "midp2"
        } else {
            "midp1"
        };
        out.add(id, 1.0, vec![format!("MicroEdition-Profile: {profile}")]);
    }

    let midlets = manifest.midlet_classes();
    if !midlets.is_empty() {
        out.add(
            "midlet_entry",
            1.0,
            midlets
                .iter()
                .map(|c| format!("MIDlet class: {c}"))
                .collect(),
        );
    }
}

fn detect_text(archive: &Archive, out: &mut CapabilityManifest) {
    let mut literal_count = 0usize;
    let mut undecodable = 0usize;
    let mut classes_parsed = 0usize;

    for entry in archive.classes() {
        let Ok(class) = ClassFile::parse(&entry.data) else {
            continue;
        };
        classes_parsed += 1;
        for literal in class.string_literals() {
            literal_count += 1;
            if literal.decoded.is_none() {
                undecodable += 1;
            }
        }
    }

    if literal_count > 0 {
        out.add(
            "constant_pool_text",
            1.0,
            vec![format!(
                "{literal_count} string literals across {classes_parsed} classes"
            )],
        );
    }

    // Literals that are not valid modified UTF-8 are the signature of a game storing text in its
    // own charset and decoding it at runtime, which needs a charset table before anything can be
    // translated.
    if undecodable > 0 {
        let ratio = undecodable as f32 / literal_count.max(1) as f32;
        out.add(
            "custom_charset",
            ratio.clamp(0.3, 1.0),
            vec![format!(
                "{undecodable} of {literal_count} literals are not valid modified UTF-8"
            )],
        );
    }
}

fn detect_resources(archive: &Archive, out: &mut CapabilityManifest) {
    let mut images = Vec::new();
    let mut text_files = Vec::new();
    let mut binaries = Vec::new();

    for entry in archive.entries() {
        match entry.extension().as_str() {
            "png" | "jpg" | "jpeg" | "gif" => images.push(entry.name.clone()),
            "txt" | "properties" | "xml" | "json" | "csv" => text_files.push(entry.name.clone()),
            "dat" | "bin" | "res" | "pak" => binaries.push(entry.name.clone()),
            _ => {}
        }
    }

    if !images.is_empty() {
        out.add(
            "image_assets",
            1.0,
            vec![format!("{} image resources", images.len())],
        );
        // Small images in quantity are usually glyph sheets or sprite atlases rather than
        // artwork; either way they are candidates for the font and asset pipelines.
        let small = images.len() >= 4;
        if small {
            out.add(
                "bitmap_font_candidates",
                0.4,
                vec![format!(
                    "{} images present; glyph sheets cannot be confirmed without pixel analysis",
                    images.len()
                )],
            );
        }
    }
    if !text_files.is_empty() {
        out.add(
            "resource_text",
            1.0,
            text_files.iter().take(8).cloned().collect(),
        );
    }
    if !binaries.is_empty() {
        out.add(
            "opaque_resources",
            0.6,
            binaries.iter().take(8).cloned().collect(),
        );
    }
}

fn detect_obfuscation(archive: &Archive, out: &mut CapabilityManifest) {
    let mut short_names = 0usize;
    let mut total = 0usize;

    for entry in archive.classes() {
        total += 1;
        let stem = entry
            .name
            .rsplit('/')
            .next()
            .unwrap_or(&entry.name)
            .trim_end_matches(".class");
        if stem.len() <= 2 {
            short_names += 1;
        }
    }

    if total > 0 && short_names * 2 > total {
        // Obfuscation does not block localization, but it does mean patches must be anchored to
        // detected structure rather than to class names, which will differ between builds.
        out.add(
            "obfuscated_names",
            (short_names as f32 / total as f32).min(1.0),
            vec![format!(
                "{short_names} of {total} classes have names of 2 characters or fewer"
            )],
        );
    }
}
