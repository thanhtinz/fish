//! Java class file reading and constant-pool rewriting.
//!
//! The localizer only ever needs to change *text*, and in a class file all text lives in the
//! constant pool. So this parses the pool and records where it ends, then treats everything after
//! it as an opaque tail that is copied through byte for byte. Nothing here parses methods, fields
//! or attributes.
//!
//! That is not a shortcut, it is the safety property: a localized class differs from the original
//! only in the bytes of the Utf8 entries that were translated. Bytecode, the exception table, the
//! stack map and every offset inside them are untouched, so no amount of translation can produce
//! an invalid method body.

use crate::error::{Error, Result};

const MAGIC: u32 = 0xCAFE_BABE;

/// One constant pool entry.
///
/// Non-Utf8 entries keep their payload as raw bytes: the localizer never inspects them, and
/// round-tripping them verbatim is both simpler and safer than modelling every tag.
#[derive(Debug, Clone)]
pub enum Constant {
    /// Text. `raw` is modified UTF-8 as stored on disk; `decoded` is `None` when those bytes are
    /// not valid modified UTF-8, which is common in J2ME games that keep text in a custom charset.
    Utf8 {
        raw: Vec<u8>,
        decoded: Option<String>,
    },
    /// `CONSTANT_String`, pointing at the Utf8 entry holding the literal.
    StringRef { utf8_index: u16 },
    /// Any other tag, kept exactly as read.
    Other { tag: u8, payload: Vec<u8> },
    /// The unusable slot that follows a Long or Double (JVMS 4.4.5).
    Unusable,
}

impl Constant {
    fn kind(&self) -> &'static str {
        match self {
            Constant::Utf8 { .. } => "Utf8",
            Constant::StringRef { .. } => "String",
            Constant::Other { .. } => "other constant",
            Constant::Unusable => "unusable slot",
        }
    }
}

/// A string literal the game can display, and where it lives.
///
/// `utf8_index` is the patch target. `string_index` is kept because it is what bytecode actually
/// references, which is how a literal is traced back to the code that shows it.
#[derive(Debug, Clone)]
pub struct StringLiteral {
    pub string_index: u16,
    pub utf8_index: u16,
    pub raw: Vec<u8>,
    pub decoded: Option<String>,
}

/// A parsed class file.
#[derive(Debug)]
pub struct ClassFile {
    pub minor_version: u16,
    pub major_version: u16,
    /// One-based, matching JVMS numbering. Index 0 is a placeholder and never read.
    constants: Vec<Constant>,
    /// Everything after the constant pool, copied verbatim on write.
    tail: Vec<u8>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.pos + n > self.bytes.len() {
            return Err(Error::Truncated {
                offset: self.pos,
                needed: n,
                available: self.bytes.len().saturating_sub(self.pos),
            });
        }
        let slice = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn u1(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u2(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn u4(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}

impl ClassFile {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader { bytes, pos: 0 };

        let magic = r.u4()?;
        if magic != MAGIC {
            return Err(Error::NotAClassFile { found: magic });
        }
        let minor_version = r.u2()?;
        let major_version = r.u2()?;

        let count = r.u2()?;
        let mut constants = Vec::with_capacity(count as usize);
        constants.push(Constant::Unusable); // index 0 is not a real entry

        let mut index = 1u16;
        while index < count {
            let tag = r.u1()?;
            let constant = match tag {
                1 => {
                    let len = r.u2()? as usize;
                    let raw = r.take(len)?.to_vec();
                    let decoded = decode_modified_utf8(&raw).ok();
                    Constant::Utf8 { raw, decoded }
                }
                8 => Constant::StringRef {
                    utf8_index: r.u2()?,
                },
                // Fixed-width payloads, kept opaque.
                3 | 4 => Constant::Other {
                    tag,
                    payload: r.take(4)?.to_vec(),
                },
                5 | 6 => Constant::Other {
                    tag,
                    payload: r.take(8)?.to_vec(),
                },
                7 | 16 | 19 | 20 => Constant::Other {
                    tag,
                    payload: r.take(2)?.to_vec(),
                },
                9 | 10 | 11 | 12 | 17 | 18 => Constant::Other {
                    tag,
                    payload: r.take(4)?.to_vec(),
                },
                15 => Constant::Other {
                    tag,
                    payload: r.take(3)?.to_vec(),
                },
                _ => return Err(Error::UnknownConstantTag { tag, index }),
            };

            // Long and Double occupy two pool slots; the second is unusable (JVMS 4.4.5).
            let wide = matches!(constant, Constant::Other { tag: 5 | 6, .. });
            constants.push(constant);
            index += 1;
            if wide {
                constants.push(Constant::Unusable);
                index += 1;
            }
        }

        let tail = bytes[r.pos..].to_vec();
        Ok(ClassFile {
            minor_version,
            major_version,
            constants,
            tail,
        })
    }

    /// Number of pool slots, matching the `constant_pool_count` field (entries + 1).
    pub fn constant_count(&self) -> u16 {
        self.constants.len() as u16
    }

    pub fn constant(&self, index: u16) -> Result<&Constant> {
        self.constants
            .get(index as usize)
            .ok_or(Error::ConstantIndexOutOfRange {
                index,
                count: self.constant_count(),
            })
    }

    /// Every literal the game could display: the Utf8 entries a `CONSTANT_String` points at.
    ///
    /// Filtering by `CONSTANT_String` is what separates displayable text from the class names,
    /// field names and type descriptors that share the pool. Guessing instead - scoring a Utf8
    /// entry on whether it "looks like a sentence" - would both miss short labels and rename
    /// classes, which breaks the class irrecoverably.
    pub fn string_literals(&self) -> Vec<StringLiteral> {
        let mut out = Vec::new();
        for (i, constant) in self.constants.iter().enumerate() {
            let Constant::StringRef { utf8_index } = constant else {
                continue;
            };
            if let Some(Constant::Utf8 { raw, decoded }) = self.constants.get(*utf8_index as usize)
            {
                out.push(StringLiteral {
                    string_index: i as u16,
                    utf8_index: *utf8_index,
                    raw: raw.clone(),
                    decoded: decoded.clone(),
                });
            }
        }
        out
    }

    /// Replaces the bytes of a Utf8 entry.
    ///
    /// The new text can be any length: the pool is re-serialised on write, and because nothing
    /// outside the pool refers to a constant by byte offset - only by index - lengths may change
    /// freely. This is why the localizer can put longer Vietnamese text into a class at all.
    pub fn set_utf8_raw(&mut self, index: u16, raw: Vec<u8>) -> Result<()> {
        if raw.len() > u16::MAX as usize {
            return Err(Error::Utf8TooLong { len: raw.len() });
        }
        let count = self.constant_count();
        let slot = self
            .constants
            .get_mut(index as usize)
            .ok_or(Error::ConstantIndexOutOfRange { index, count })?;
        match slot {
            Constant::Utf8 { .. } => {
                let decoded = decode_modified_utf8(&raw).ok();
                *slot = Constant::Utf8 { raw, decoded };
                Ok(())
            }
            other => Err(Error::ConstantTypeMismatch {
                index,
                expected: "Utf8",
                actual: other.kind(),
            }),
        }
    }

    /// Convenience wrapper that stores `text` as modified UTF-8.
    pub fn set_utf8_text(&mut self, index: u16, text: &str) -> Result<()> {
        self.set_utf8_raw(index, encode_modified_utf8(text))
    }

    /// Serialises the class back out.
    pub fn write(&self) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(self.tail.len() + 512);
        out.extend_from_slice(&MAGIC.to_be_bytes());
        out.extend_from_slice(&self.minor_version.to_be_bytes());
        out.extend_from_slice(&self.major_version.to_be_bytes());
        out.extend_from_slice(&self.constant_count().to_be_bytes());

        for constant in self.constants.iter().skip(1) {
            match constant {
                Constant::Utf8 { raw, .. } => {
                    out.push(1);
                    out.extend_from_slice(&(raw.len() as u16).to_be_bytes());
                    out.extend_from_slice(raw);
                }
                Constant::StringRef { utf8_index } => {
                    out.push(8);
                    out.extend_from_slice(&utf8_index.to_be_bytes());
                }
                Constant::Other { tag, payload } => {
                    out.push(*tag);
                    out.extend_from_slice(payload);
                }
                // Written by the Long or Double before it, so it contributes no bytes.
                Constant::Unusable => {}
            }
        }

        out.extend_from_slice(&self.tail);
        Ok(out)
    }
}

/// Decodes JVM modified UTF-8 (JVMS 4.4.7).
///
/// This is not standard UTF-8: NUL is encoded as two bytes so it never appears inside a string,
/// and characters outside the BMP are stored as a surrogate pair encoded separately rather than
/// as one four-byte sequence. Decoding with a standard UTF-8 reader therefore rejects valid class
/// files, which would make the tool report perfectly good games as corrupt.
pub fn decode_modified_utf8(bytes: &[u8]) -> Result<String> {
    let mut out = String::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0 {
            // A bare NUL is never produced by a conforming encoder.
            return Err(Error::MalformedModifiedUtf8 { offset: i });
        } else if b < 0x80 {
            out.push(b as char);
            i += 1;
        } else if b & 0xE0 == 0xC0 {
            let b2 = *bytes
                .get(i + 1)
                .ok_or(Error::MalformedModifiedUtf8 { offset: i })?;
            if b2 & 0xC0 != 0x80 {
                return Err(Error::MalformedModifiedUtf8 { offset: i + 1 });
            }
            let code = (((b & 0x1F) as u32) << 6) | ((b2 & 0x3F) as u32);
            out.push(char::from_u32(code).ok_or(Error::MalformedModifiedUtf8 { offset: i })?);
            i += 2;
        } else if b & 0xF0 == 0xE0 {
            let b2 = *bytes
                .get(i + 1)
                .ok_or(Error::MalformedModifiedUtf8 { offset: i })?;
            let b3 = *bytes
                .get(i + 2)
                .ok_or(Error::MalformedModifiedUtf8 { offset: i })?;
            if b2 & 0xC0 != 0x80 || b3 & 0xC0 != 0x80 {
                return Err(Error::MalformedModifiedUtf8 { offset: i + 1 });
            }
            let code =
                (((b & 0x0F) as u32) << 12) | (((b2 & 0x3F) as u32) << 6) | ((b3 & 0x3F) as u32);
            // Surrogates are legal here: a supplementary character is stored as an encoded pair.
            match char::from_u32(code) {
                Some(c) => out.push(c),
                None => out.push(char::REPLACEMENT_CHARACTER),
            }
            i += 3;
        } else {
            return Err(Error::MalformedModifiedUtf8 { offset: i });
        }
    }
    Ok(out)
}

/// Encodes to JVM modified UTF-8.
pub fn encode_modified_utf8(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 8);
    for c in text.chars() {
        let code = c as u32;
        if code == 0 {
            out.extend_from_slice(&[0xC0, 0x80]);
        } else if code < 0x80 {
            out.push(code as u8);
        } else if code < 0x800 {
            out.push(0xC0 | (code >> 6) as u8);
            out.push(0x80 | (code & 0x3F) as u8);
        } else if code < 0x1_0000 {
            out.push(0xE0 | (code >> 12) as u8);
            out.push(0x80 | ((code >> 6) & 0x3F) as u8);
            out.push(0x80 | (code & 0x3F) as u8);
        } else {
            // Supplementary: emit the surrogate pair, each as three bytes.
            let v = code - 0x1_0000;
            let high = 0xD800 + (v >> 10);
            let low = 0xDC00 + (v & 0x3FF);
            for s in [high, low] {
                out.push(0xE0 | (s >> 12) as u8);
                out.push(0x80 | ((s >> 6) & 0x3F) as u8);
                out.push(0x80 | (s & 0x3F) as u8);
            }
        }
    }
    out
}
