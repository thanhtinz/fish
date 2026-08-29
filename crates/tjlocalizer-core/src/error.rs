use std::path::PathBuf;

/// Every failure the core can produce.
///
/// Deliberately specific: this tool reads untrusted third-party archives, so "something went
/// wrong" is not good enough to tell a malformed class apart from an unsupported one.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a Java class file: expected magic 0xCAFEBABE, found {found:#010x}")]
    NotAClassFile { found: u32 },

    #[error(
        "class file truncated: needed {needed} bytes at offset {offset}, {available} available"
    )]
    Truncated {
        offset: usize,
        needed: usize,
        available: usize,
    },

    #[error("unsupported constant pool tag {tag} at index {index}")]
    UnknownConstantTag { tag: u8, index: u16 },

    #[error("constant pool index {index} is out of range (pool holds {count} entries)")]
    ConstantIndexOutOfRange { index: u16, count: u16 },

    #[error("constant pool entry {index} is a {actual}, expected {expected}")]
    ConstantTypeMismatch {
        index: u16,
        expected: &'static str,
        actual: &'static str,
    },

    #[error("a Utf8 constant may hold at most 65535 bytes, this one needs {len}")]
    Utf8TooLong { len: usize },

    #[error("malformed modified UTF-8 at byte {offset}")]
    MalformedModifiedUtf8 { offset: usize },

    #[error("archive entry '{name}' escapes the output directory")]
    UnsafeEntryPath { name: String },

    #[error("archive entry '{name}' is {size} bytes, over the {limit} byte limit")]
    EntryTooLarge { name: String, size: u64, limit: u64 },

    #[error("archive holds {count} entries, over the {limit} entry limit")]
    TooManyEntries { count: usize, limit: usize },

    #[error("no MIDlet entry point found in the manifest")]
    NoMidletFound,

    #[error("{path} is not a valid project: {reason}")]
    InvalidProject { path: PathBuf, reason: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
