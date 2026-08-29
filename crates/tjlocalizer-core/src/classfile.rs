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

/// One instruction that loads a string constant, and where its operand is.
///
/// `operand` is an offset into the class body as this build stores it, which is stable because
/// everything after the constant pool is kept verbatim and constants are only ever appended.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeSite {
    pub method: String,
    pub descriptor: String,
    /// Offset of the operand within the class body.
    operand: usize,
    /// One byte for `ldc`, two for `ldc_w`.
    operand_width: usize,
    /// Offset within the method's code, which is what a disassembler prints.
    pub pc: usize,
    pub string_index: u16,
    pub utf8_index: u16,
    /// The text loaded here, where the bytes decode.
    pub text: Option<String>,
}

struct Method {
    name: String,
    descriptor: String,
    code: Option<Code>,
}

struct Code {
    /// Where the bytecode starts within the class body.
    at: usize,
    len: usize,
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

    /// Every `CONSTANT_Integer` in the pool, with its index.
    ///
    /// Games keep layout numbers here - how many columns a glyph sheet has, how wide a cell is -
    /// and those are the numbers a font swap has to change. Reading them is how a rule can say
    /// "this game holds a 16 where I expect one" before it changes anything.
    pub fn integers(&self) -> Vec<(u16, i32)> {
        self.constants
            .iter()
            .enumerate()
            .filter_map(|(i, c)| match c {
                Constant::Other { tag: 3, payload } => {
                    let bytes: [u8; 4] = payload.as_slice().try_into().ok()?;
                    Some((i as u16, i32::from_be_bytes(bytes)))
                }
                _ => None,
            })
            .collect()
    }

    /// Replaces the value of a `CONSTANT_Integer`.
    ///
    /// Four bytes for four bytes, so nothing about the class's shape changes - the same reason
    /// the pool can be rewritten at all. It is still a change to what the game computes, which is
    /// why the rule engine will only do it where a rule said which value it expected to find.
    pub fn set_integer(&mut self, index: u16, value: i32) -> Result<()> {
        let count = self.constant_count();
        let slot = self
            .constants
            .get_mut(index as usize)
            .ok_or(Error::ConstantIndexOutOfRange { index, count })?;
        match slot {
            Constant::Other { tag: 3, payload } => {
                *payload = value.to_be_bytes().to_vec();
                Ok(())
            }
            other => Err(Error::ConstantTypeMismatch {
                index,
                expected: "Integer",
                actual: other.kind(),
            }),
        }
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

    /// One place in a method's bytecode where a string constant is loaded.
    ///
    /// The distinction this makes possible is the whole reason it exists. Rewriting a Utf8
    /// constant changes the text everywhere it is used; a game that shows `Back` on eleven
    /// screens has one constant for all eleven, and a translation that has to differ on one of
    /// them cannot be expressed in the pool at all. A site is one of those eleven.
    pub fn string_sites(&self) -> Result<Vec<CodeSite>> {
        let mut sites = Vec::new();
        for method in self.methods()? {
            let Some(code) = method.code else {
                continue;
            };
            let mut pc = 0usize;
            while pc < code.len {
                let opcode = self.tail[code.at + pc];
                let (constant, width) = match opcode {
                    // ldc: a one-byte index, so it can only reach the first 255 constants.
                    0x12 => (u16::from(self.tail[code.at + pc + 1]), 1usize),
                    // ldc_w: two bytes.
                    0x13 => (
                        u16::from_be_bytes([
                            self.tail[code.at + pc + 1],
                            self.tail[code.at + pc + 2],
                        ]),
                        2,
                    ),
                    _ => {
                        pc += instruction_length(&self.tail[code.at..code.at + code.len], pc)?;
                        continue;
                    }
                };
                // Only string loads. `ldc` also loads ints, floats and class references, and
                // those are not text however much a translator would like them to be.
                if let Ok(Constant::StringRef { utf8_index }) = self.constant(constant) {
                    sites.push(CodeSite {
                        method: method.name.clone(),
                        descriptor: method.descriptor.clone(),
                        operand: code.at + pc + 1,
                        operand_width: width,
                        pc,
                        string_index: constant,
                        utf8_index: *utf8_index,
                        text: match self.constant(*utf8_index) {
                            Ok(Constant::Utf8 { decoded, .. }) => decoded.clone(),
                            _ => None,
                        },
                    });
                }
                pc += instruction_length(&self.tail[code.at..code.at + code.len], pc)?;
            }
        }
        Ok(sites)
    }

    /// Adds a `CONSTANT_String` and the Utf8 behind it, returning the String's index.
    ///
    /// Appending only: every existing index keeps meaning what it meant, which is what lets the
    /// rest of the class stay verbatim.
    pub fn add_string(&mut self, text: &str) -> Result<u16> {
        let raw = encode_modified_utf8(text);
        if raw.len() > u16::MAX as usize {
            return Err(Error::Utf8TooLong { len: raw.len() });
        }
        let decoded = decode_modified_utf8(&raw).ok();
        self.constants.push(Constant::Utf8 { raw, decoded });
        let utf8_index = (self.constants.len() - 1) as u16;
        self.constants.push(Constant::StringRef { utf8_index });
        Ok((self.constants.len() - 1) as u16)
    }

    /// Adds a `CONSTANT_Integer`, returning its index.
    ///
    /// Appending only, like `add_string`, so every existing index keeps meaning what it meant.
    pub fn add_integer(&mut self, value: i32) -> u16 {
        self.constants.push(Constant::Other {
            tag: 3,
            payload: value.to_be_bytes().to_vec(),
        });
        (self.constants.len() - 1) as u16
    }

    /// Points one load instruction at a different constant.
    ///
    /// The instruction keeps its length, so every jump offset, every exception range and every
    /// stack map frame in the method stays correct - which is the only reason this is safe to do
    /// at all. Where the new index will not fit the instruction it is refused: widening an `ldc`
    /// to an `ldc_w` moves everything after it, and a class whose jumps are one byte out is a
    /// class that fails verification in a way nobody can debug from a translated string.
    pub fn point_site_at(&mut self, site: &CodeSite, string_index: u16) -> Result<()> {
        if self.constant(string_index).is_err() {
            return Err(Error::ConstantIndexOutOfRange {
                index: string_index,
                count: self.constant_count(),
            });
        }
        match site.operand_width {
            1 => {
                if string_index > u8::MAX as u16 {
                    return Err(Error::PoolTooFullForSite {
                        count: self.constant_count(),
                    });
                }
                self.tail[site.operand] = string_index as u8;
            }
            _ => {
                let bytes = string_index.to_be_bytes();
                self.tail[site.operand] = bytes[0];
                self.tail[site.operand + 1] = bytes[1];
            }
        }
        Ok(())
    }

    /// Walks the class body far enough to find each method and its code.
    fn methods(&self) -> Result<Vec<Method>> {
        let mut r = Reader {
            bytes: &self.tail,
            pos: 0,
        };
        let bad = |reason: &str| Error::MalformedClassBody {
            reason: reason.to_string(),
        };

        r.u2().map_err(|_| bad("no access flags"))?;
        r.u2().map_err(|_| bad("no this_class"))?;
        r.u2().map_err(|_| bad("no super_class"))?;
        let interfaces = r.u2().map_err(|_| bad("no interface count"))?;
        r.take(interfaces as usize * 2)
            .map_err(|_| bad("interfaces truncated"))?;

        // Fields first: their attributes have the same shape as a method's, and skipping them
        // wrongly would put the reader in the middle of a byte stream that still parses.
        let fields = r.u2().map_err(|_| bad("no field count"))?;
        for _ in 0..fields {
            r.take(6).map_err(|_| bad("field truncated"))?;
            skip_attributes(&mut r)?;
        }

        let count = r.u2().map_err(|_| bad("no method count"))?;
        let mut methods = Vec::new();
        for _ in 0..count {
            r.u2().map_err(|_| bad("method truncated"))?;
            let name_index = r.u2().map_err(|_| bad("method truncated"))?;
            let descriptor_index = r.u2().map_err(|_| bad("method truncated"))?;

            let attributes = r.u2().map_err(|_| bad("no attribute count"))?;
            let mut code = None;
            for _ in 0..attributes {
                let attribute_name = r.u2().map_err(|_| bad("attribute truncated"))?;
                let length = r.u4().map_err(|_| bad("attribute truncated"))? as usize;
                let at = r.pos;
                r.take(length).map_err(|_| bad("attribute truncated"))?;
                if self.utf8_at(attribute_name).as_deref() != Some("Code") {
                    continue;
                }
                if length < 8 {
                    return Err(bad("a Code attribute shorter than its own header"));
                }
                let code_length = u32::from_be_bytes([
                    self.tail[at + 4],
                    self.tail[at + 5],
                    self.tail[at + 6],
                    self.tail[at + 7],
                ]) as usize;
                if at + 8 + code_length > self.tail.len() {
                    return Err(bad(
                        "a Code attribute claiming more code than the class holds",
                    ));
                }
                code = Some(Code {
                    at: at + 8,
                    len: code_length,
                });
            }

            methods.push(Method {
                name: self.utf8_at(name_index).unwrap_or_default(),
                descriptor: self.utf8_at(descriptor_index).unwrap_or_default(),
                code,
            });
        }
        Ok(methods)
    }

    fn utf8_at(&self, index: u16) -> Option<String> {
        match self.constants.get(index as usize) {
            Some(Constant::Utf8 { decoded, .. }) => decoded.clone(),
            _ => None,
        }
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

/// Skips a run of attributes, whatever they are.
///
/// Attributes nest - a Code attribute holds attributes of its own - and every one of them states
/// its own length, so skipping is exact and does not require understanding any of them.
fn skip_attributes(r: &mut Reader) -> Result<()> {
    let bad = |reason: &str| Error::MalformedClassBody {
        reason: reason.to_string(),
    };
    let count = r.u2().map_err(|_| bad("no attribute count"))?;
    for _ in 0..count {
        r.u2().map_err(|_| bad("attribute truncated"))?;
        let length = r.u4().map_err(|_| bad("attribute truncated"))? as usize;
        r.take(length).map_err(|_| bad("attribute truncated"))?;
    }
    Ok(())
}

/// How many bytes one instruction occupies.
///
/// A bytecode stream cannot be scanned for a byte value: the operand of one instruction is
/// indistinguishable from the opcode of another, and a scanner that ignored that would find
/// `ldc`s inside jump offsets and patch them. So every instruction is stepped over by its true
/// length, which for three of them depends on their contents.
fn instruction_length(code: &[u8], pc: usize) -> Result<usize> {
    let bad = |reason: &str| Error::MalformedClassBody {
        reason: reason.to_string(),
    };
    let opcode = *code
        .get(pc)
        .ok_or_else(|| bad("code ends mid-instruction"))?;

    let length = match opcode {
        // The three variable-length instructions, each measured rather than tabulated.
        0xC4 => {
            // wide: the instruction it modifies decides its length, and `iinc` is the one that
            // takes a second operand.
            let modified = *code
                .get(pc + 1)
                .ok_or_else(|| bad("a wide instruction with nothing after it"))?;
            if modified == 0x84 {
                6
            } else {
                4
            }
        }
        0xAA => {
            // tableswitch: padded to a four-byte boundary, then default, low, high, and one
            // offset per entry.
            let pad = (4 - ((pc + 1) % 4)) % 4;
            let base = pc + 1 + pad;
            let low = read_i32(code, base + 4).ok_or_else(|| bad("a truncated tableswitch"))?;
            let high = read_i32(code, base + 8).ok_or_else(|| bad("a truncated tableswitch"))?;
            if high < low {
                return Err(bad("a tableswitch whose high is below its low"));
            }
            let entries = (high - low + 1) as usize;
            (base + 12 + entries * 4) - pc
        }
        0xAB => {
            // lookupswitch: padding, default, count, then a pair per entry.
            let pad = (4 - ((pc + 1) % 4)) % 4;
            let base = pc + 1 + pad;
            let pairs = read_i32(code, base + 4).ok_or_else(|| bad("a truncated lookupswitch"))?;
            if pairs < 0 {
                return Err(bad("a lookupswitch with a negative pair count"));
            }
            (base + 8 + pairs as usize * 8) - pc
        }
        _ => 1 + operand_bytes(opcode),
    };

    if pc + length > code.len() {
        return Err(bad("an instruction running past the end of its method"));
    }
    Ok(length)
}

fn read_i32(code: &[u8], at: usize) -> Option<i32> {
    let bytes = code.get(at..at + 4)?;
    Some(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

/// Operand bytes for every fixed-length opcode (JVMS 6.5).
///
/// Written out rather than derived, because the JVM's encoding is not derivable: the lengths are
/// a historical list and the only way to be right about them is to have the list.
fn operand_bytes(opcode: u8) -> usize {
    match opcode {
        // bipush, ldc, and the single-byte-index loads and stores.
        0x10 | 0x12 | 0x15..=0x19 | 0x36..=0x3A | 0xA9 | 0xBC => 1,
        // sipush, ldc_w, ldc2_w, iinc, the jumps, the field and method references, new,
        // anewarray, checkcast, instanceof, ifnull and ifnonnull.
        0x11
        | 0x13
        | 0x14
        | 0x84
        | 0x99..=0xA8
        | 0xB2..=0xB8
        | 0xBB
        | 0xBD
        | 0xC0
        | 0xC1
        | 0xC6
        | 0xC7 => 2,
        // multianewarray: a type and a dimension count.
        0xC5 => 3,
        // invokeinterface and invokedynamic carry two bytes nobody reads any more.
        0xB9 | 0xBA => 4,
        // goto_w and jsr_w.
        0xC8 | 0xC9 => 4,
        _ => 0,
    }
}
