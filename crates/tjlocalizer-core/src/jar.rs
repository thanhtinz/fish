//! JAR/JAD reading, writing and manifest handling.
//!
//! Every archive reaching this module is untrusted third-party input (specification §29), so the
//! reader enforces hard limits before allocating anything: entry count, per-entry size, total
//! uncompressed size, and path safety. A localization tool that can be made to fill the disk or
//! write outside its output directory by a crafted game file is not usable.

use crate::error::{Error, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;

/// Limits applied to every archive read. Generous for real J2ME games, which are typically
/// well under a megabyte, while still bounding what a malicious archive can cost.
#[derive(Debug, Clone)]
pub struct ArchiveLimits {
    pub max_entries: usize,
    pub max_entry_size: u64,
    pub max_total_size: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_entries: 20_000,
            max_entry_size: 64 * 1024 * 1024,
            max_total_size: 512 * 1024 * 1024,
        }
    }
}

/// One file inside the archive.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub name: String,
    pub data: Vec<u8>,
}

impl ArchiveEntry {
    pub fn is_class(&self) -> bool {
        self.name.ends_with(".class")
    }

    /// Lowercase extension, or an empty string.
    pub fn extension(&self) -> String {
        Path::new(&self.name)
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    }
}

/// A JAR loaded into memory, with entry order preserved.
#[derive(Debug)]
pub struct Archive {
    entries: Vec<ArchiveEntry>,
    pub sha256: String,
}

impl Archive {
    pub fn read(bytes: &[u8]) -> Result<Self> {
        Self::read_with_limits(bytes, &ArchiveLimits::default())
    }

    pub fn read_with_limits(bytes: &[u8], limits: &ArchiveLimits) -> Result<Self> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
        if zip.len() > limits.max_entries {
            return Err(Error::TooManyEntries {
                count: zip.len(),
                limit: limits.max_entries,
            });
        }

        let mut entries = Vec::with_capacity(zip.len());
        let mut total = 0u64;

        for i in 0..zip.len() {
            let file = zip.by_index(i)?;
            if file.is_dir() {
                continue;
            }

            // `enclosed_name` is None for absolute paths and for anything containing `..`, which
            // is exactly the traversal case. Checked before reading so a hostile entry costs
            // nothing.
            let name = file
                .enclosed_name()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .ok_or_else(|| Error::UnsafeEntryPath {
                    name: file.name().to_string(),
                })?;

            // The declared size is checked first so a zip bomb is refused before it is expanded,
            // and the real length is checked after in case the header lied.
            let declared = file.size();
            if declared > limits.max_entry_size {
                return Err(Error::EntryTooLarge {
                    name,
                    size: declared,
                    limit: limits.max_entry_size,
                });
            }

            let mut data = Vec::with_capacity(declared.min(1 << 20) as usize);
            file.take(limits.max_entry_size + 1).read_to_end(&mut data)?;
            if data.len() as u64 > limits.max_entry_size {
                return Err(Error::EntryTooLarge {
                    name,
                    size: data.len() as u64,
                    limit: limits.max_entry_size,
                });
            }

            total += data.len() as u64;
            if total > limits.max_total_size {
                return Err(Error::EntryTooLarge {
                    name,
                    size: total,
                    limit: limits.max_total_size,
                });
            }

            entries.push(ArchiveEntry { name, data });
        }

        Ok(Archive {
            entries,
            sha256: sha256_hex(bytes),
        })
    }

    pub fn entries(&self) -> &[ArchiveEntry] {
        &self.entries
    }

    pub fn get(&self, name: &str) -> Option<&ArchiveEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    pub fn replace(&mut self, name: &str, data: Vec<u8>) -> bool {
        match self.entries.iter_mut().find(|e| e.name == name) {
            Some(entry) => {
                entry.data = data;
                true
            }
            None => false,
        }
    }

    /// Drops an entry, returning whether it was there.
    ///
    /// Used by validation tests and by profiles that strip an obsolete resource; the remaining
    /// entry order is untouched so the build stays reproducible.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.name != name);
        self.entries.len() != before
    }

    pub fn insert(&mut self, name: impl Into<String>, data: Vec<u8>) {
        let name = name.into();
        if !self.replace(&name, data.clone()) {
            self.entries.push(ArchiveEntry { name, data });
        }
    }

    pub fn classes(&self) -> impl Iterator<Item = &ArchiveEntry> {
        self.entries.iter().filter(|e| e.is_class())
    }

    /// Writes the archive back out.
    ///
    /// Entry order is preserved and timestamps are fixed, so building the same inputs twice
    /// produces byte-identical output. The specification asks for reproducible builds, and a
    /// build that embeds the current time cannot be diffed or verified by hash.
    pub fn write(&self) -> Result<Vec<u8>> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .last_modified_time(
                    zip::DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0)
                        .expect("fixed timestamp is valid"),
                );
            for entry in &self.entries {
                zip.start_file(entry.name.clone(), options)?;
                zip.write_all(&entry.data)?;
            }
            zip.finish()?;
        }
        Ok(buffer.into_inner())
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A parsed JAR manifest or JAD descriptor.
///
/// Both are the same `Key: value` format, so one parser serves both. Key order is kept
/// alphabetical for stable output; the JAR specification does not assign order any meaning
/// beyond `Manifest-Version` coming first, which `render` handles.
#[derive(Debug, Clone, Default)]
pub struct Manifest {
    values: BTreeMap<String, String>,
}

impl Manifest {
    /// Parses manifest text, joining continuation lines.
    ///
    /// The format wraps at 72 bytes and continues with a single leading space (JAR spec). A
    /// parser that treats each physical line as a record silently truncates long MIDlet
    /// declarations, which is where the entry-point class name lives.
    pub fn parse(text: &str) -> Self {
        let mut values = BTreeMap::new();
        let mut current: Option<(String, String)> = None;

        for line in text.lines() {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix(' ') {
                if let Some((_, value)) = current.as_mut() {
                    value.push_str(rest);
                }
                continue;
            }
            if let Some((key, value)) = current.take() {
                values.insert(key, value);
            }
            if let Some((key, value)) = line.split_once(':') {
                current = Some((key.trim().to_string(), value.trim().to_string()));
            }
        }
        if let Some((key, value)) = current {
            values.insert(key, value);
        }
        Manifest { values }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.values.iter()
    }

    /// The MIDlet entry-point class names declared by `MIDlet-<n>` attributes.
    ///
    /// Each is `name, icon, class`. Returning the class lets the build stage confirm the entry
    /// point still exists after patching, which is the single most common way a repackaged JAR
    /// fails to launch.
    pub fn midlet_classes(&self) -> Vec<String> {
        let mut out = Vec::new();
        for n in 1..=32 {
            let Some(value) = self.get(&format!("MIDlet-{n}")) else {
                continue;
            };
            if let Some(class) = value.split(',').nth(2) {
                let class = class.trim();
                if !class.is_empty() {
                    out.push(class.to_string());
                }
            }
        }
        out
    }

    /// Renders back to manifest text, wrapping at 72 bytes as the format requires.
    pub fn render(&self) -> String {
        let mut out = String::new();
        if let Some(version) = self.get("Manifest-Version") {
            out.push_str(&wrap_header("Manifest-Version", version));
        }
        for (key, value) in &self.values {
            if key == "Manifest-Version" {
                continue;
            }
            out.push_str(&wrap_header(key, value));
        }
        out.push('\n');
        out
    }
}

/// Wraps one header at 72 bytes, continuing with a leading space.
///
/// Splitting is done on byte count but only at character boundaries: a MIDlet name containing
/// non-ASCII would otherwise be cut mid-character and become mojibake on the device.
fn wrap_header(key: &str, value: &str) -> String {
    let line = format!("{key}: {value}");
    let mut out = String::with_capacity(line.len() + 8);
    let mut budget = 72;
    let mut written = 0;

    for c in line.chars() {
        let len = c.len_utf8();
        if written + len > budget {
            out.push('\n');
            out.push(' ');
            written = 1;
            budget = 72;
        }
        out.push(c);
        written += len;
    }
    out.push('\n');
    out
}
