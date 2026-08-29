//! Unreal Engine's compiled string table (`.locres`).
//!
//! The format a great many Steam games keep their text in. It is self-contained and documented -
//! a header, a table of namespaces and keys, and an array of the strings themselves - which makes
//! it one of the few binary game formats worth reading directly rather than through the engine.
//!
//! Two things are done deliberately here.
//!
//! **It refuses rather than guesses.** Anything whose magic or version is not exactly what this
//! parser knows is rejected with a reason, not read on a hopeful interpretation. A binary format
//! read slightly wrong produces text that looks almost right and a file that crashes a game, and
//! the second is discovered long after the first.
//!
//! **Everything it does not translate is carried through.** Namespaces, keys and the source-text
//! hashes are written back exactly as they were read. The hashes in particular are Unreal's way
//! of noticing that a translation is stale, and inventing new ones would tell the engine that
//! every string had been re-checked against a source nobody looked at.

use crate::{Error, Result};

/// The 16 bytes that begin every non-legacy `.locres`.
const MAGIC: [u8; 16] = [
    0x0E, 0x14, 0x74, 0x75, 0x67, 0x4A, 0x03, 0xFC, 0x4A, 0x15, 0x90, 0x9D, 0xC3, 0x37, 0x7F, 0x1B,
];

/// The versions this build understands.
///
/// 2 (`Optimized`) and 3 (`OptimizedCityHash64UTF16`) share a layout; 3 adds a count of entries
/// that is written and otherwise unused. Version 1 keeps its strings inline rather than in a
/// shared array, and is not read here rather than being read badly.
const OPTIMIZED: u8 = 2;
const OPTIMIZED_CITY_HASH: u8 = 3;

/// One translatable entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub namespace: String,
    pub key: String,
    pub text: String,
    /// Unreal's hash of the *source* text, kept so the engine can still tell whether a
    /// translation has gone stale.
    pub source_hash: u32,
}

/// A parsed table, holding everything needed to write it back.
#[derive(Debug, Clone)]
pub struct Locres {
    version: u8,
    namespaces: Vec<Namespace>,
    strings: Vec<(String, i32)>,
}

#[derive(Debug, Clone)]
struct Namespace {
    hash: u32,
    name: String,
    keys: Vec<Key>,
}

#[derive(Debug, Clone)]
struct Key {
    hash: u32,
    name: String,
    source_hash: u32,
    string_index: i32,
}

impl Locres {
    /// Whether these bytes are a `.locres` this build can read.
    pub fn looks_like(bytes: &[u8]) -> bool {
        bytes.len() > MAGIC.len() && bytes[..MAGIC.len()] == MAGIC
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader { bytes, pos: 0 };
        if r.take(MAGIC.len())? != MAGIC {
            return Err(invalid("this is not a .locres, or it is the legacy format"));
        }
        let version = r.u1()?;
        if version != OPTIMIZED && version != OPTIMIZED_CITY_HASH {
            return Err(invalid(&format!(
                "version {version} is not one this build reads; it reads {OPTIMIZED} and \
                 {OPTIMIZED_CITY_HASH}"
            )));
        }

        // The strings live at the end, and the header says where. Read them first: the table that
        // follows refers to them by index, and a table pointing at strings nobody read is not
        // something to notice halfway through writing a file back out.
        let strings_at = r.i8()? as usize;
        let mut s = Reader {
            bytes,
            pos: strings_at,
        };
        let count = s.i4()?;
        let mut strings = Vec::with_capacity(count.max(0) as usize);
        for _ in 0..count.max(0) {
            let text = s.string()?;
            let refs = s.i4()?;
            strings.push((text, refs));
        }

        if version >= OPTIMIZED_CITY_HASH {
            let _entries = r.u4()?; // written, and derivable from what follows
        }
        let namespace_count = r.u4()?;
        let mut namespaces = Vec::with_capacity(namespace_count as usize);
        for _ in 0..namespace_count {
            let hash = r.u4()?;
            let name = r.string()?;
            let key_count = r.u4()?;
            let mut keys = Vec::with_capacity(key_count as usize);
            for _ in 0..key_count {
                keys.push(Key {
                    hash: r.u4()?,
                    name: r.string()?,
                    source_hash: r.u4()?,
                    string_index: r.i4()?,
                });
            }
            namespaces.push(Namespace { hash, name, keys });
        }

        Ok(Locres {
            version,
            namespaces,
            strings,
        })
    }

    /// Every entry, in the order the file lists them.
    pub fn entries(&self) -> Vec<Entry> {
        let mut out = Vec::new();
        for namespace in &self.namespaces {
            for key in &namespace.keys {
                let Some((text, _)) = self.strings.get(key.string_index.max(0) as usize) else {
                    continue;
                };
                out.push(Entry {
                    namespace: namespace.name.clone(),
                    key: key.name.clone(),
                    text: text.clone(),
                    source_hash: key.source_hash,
                });
            }
        }
        out
    }

    /// Replaces the text of one entry.
    ///
    /// Two entries can share a string - Unreal stores each distinct text once and counts the
    /// references - so changing one in place would change the other with it. This gives the entry
    /// its own copy instead, and leaves the shared one for whoever else was using it.
    pub fn set(&mut self, namespace: &str, key: &str, text: &str) -> bool {
        let Some(entry) = self
            .namespaces
            .iter_mut()
            .filter(|n| n.name == namespace)
            .flat_map(|n| n.keys.iter_mut())
            .find(|k| k.name == key)
        else {
            return false;
        };

        let index = entry.string_index.max(0) as usize;
        let shared = self
            .strings
            .get(index)
            .map(|(_, refs)| *refs > 1)
            .unwrap_or(false);

        if shared {
            if let Some((_, refs)) = self.strings.get_mut(index) {
                *refs -= 1;
            }
            self.strings.push((text.to_string(), 1));
            entry.string_index = (self.strings.len() - 1) as i32;
        } else if let Some((slot, _)) = self.strings.get_mut(index) {
            *slot = text.to_string();
        } else {
            return false;
        }
        true
    }

    /// Writes the table back out.
    pub fn write(&self) -> Vec<u8> {
        // The table is written first and the strings after it, so where the strings begin is only
        // known once the table is done - which is why the header's offset is filled in last.
        let mut table = Vec::new();
        if self.version >= OPTIMIZED_CITY_HASH {
            let entries: u32 = self.namespaces.iter().map(|n| n.keys.len() as u32).sum();
            table.extend_from_slice(&entries.to_le_bytes());
        }
        table.extend_from_slice(&(self.namespaces.len() as u32).to_le_bytes());
        for namespace in &self.namespaces {
            table.extend_from_slice(&namespace.hash.to_le_bytes());
            write_string(&mut table, &namespace.name);
            table.extend_from_slice(&(namespace.keys.len() as u32).to_le_bytes());
            for key in &namespace.keys {
                table.extend_from_slice(&key.hash.to_le_bytes());
                write_string(&mut table, &key.name);
                table.extend_from_slice(&key.source_hash.to_le_bytes());
                table.extend_from_slice(&key.string_index.to_le_bytes());
            }
        }

        let mut out = Vec::with_capacity(table.len() + 64);
        out.extend_from_slice(&MAGIC);
        out.push(self.version);
        let strings_at = (out.len() + 8 + table.len()) as i64;
        out.extend_from_slice(&strings_at.to_le_bytes());
        out.extend_from_slice(&table);

        out.extend_from_slice(&(self.strings.len() as i32).to_le_bytes());
        for (text, refs) in &self.strings {
            write_string(&mut out, text);
            out.extend_from_slice(&refs.to_le_bytes());
        }
        out
    }
}

/// Unreal's string: a length, then the characters, then a terminator.
///
/// A positive length counts single bytes and a negative one counts UTF-16 code units, and the
/// count includes the terminator. Which encoding is used depends on what the string holds, so
/// both have to be read - and written, or a Vietnamese translation of an ASCII string would be
/// written as bytes that are not ASCII.
fn write_string(out: &mut Vec<u8>, text: &str) {
    if text.is_ascii() {
        out.extend_from_slice(&((text.len() + 1) as i32).to_le_bytes());
        out.extend_from_slice(text.as_bytes());
        out.push(0);
    } else {
        let units: Vec<u16> = text.encode_utf16().collect();
        out.extend_from_slice(&(-((units.len() + 1) as i32)).to_le_bytes());
        for unit in units {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        out.extend_from_slice(&[0, 0]);
    }
}

fn invalid(reason: &str) -> Error {
    Error::UnreadableResource {
        reason: reason.to_string(),
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| invalid("the file ends in the middle of a value"))?;
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u1(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u4(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i4(&mut self) -> Result<i32> {
        Ok(self.u4()? as i32)
    }

    fn i8(&mut self) -> Result<i64> {
        let b = self.take(8)?;
        Ok(i64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn string(&mut self) -> Result<String> {
        let len = self.i4()?;
        if len == 0 {
            return Ok(String::new());
        }
        if len > 0 {
            // Includes the terminator, which is not part of the text.
            let bytes = self.take(len as usize)?;
            let text = bytes.strip_suffix(&[0]).unwrap_or(bytes);
            return String::from_utf8(text.to_vec())
                .map_err(|_| invalid("a string is not valid UTF-8"));
        }
        let units = (-len) as usize;
        let bytes = self.take(units * 2)?;
        let mut wide: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        if wide.last() == Some(&0) {
            wide.pop();
        }
        String::from_utf16(&wide).map_err(|_| invalid("a string is not valid UTF-16"))
    }
}

/// How a namespace and key are written as one string.
///
/// The rest of the pipeline addresses a resource value by a single key - it is what a CSV column
/// holds and what a translator sees - and a locres entry has two parts. They are joined with `::`
/// because Unreal namespaces and keys are identifiers or asset paths, and neither ordinarily
/// contains it.
pub const SEPARATOR: &str = "::";

/// The address of one entry, as the rest of the pipeline uses it.
pub fn address(namespace: &str, key: &str) -> String {
    format!("{namespace}{SEPARATOR}{key}")
}

impl Locres {
    /// Replaces the text of the entry at a combined address.
    pub fn set_at(&mut self, address: &str, text: &str) -> bool {
        match address.split_once(SEPARATOR) {
            Some((namespace, key)) => self.set(namespace, key, text),
            None => false,
        }
    }
}
