# Architecture

## The pipeline

```
JAR  ─►  Archive  ─►  Capabilities  ─►  ContentGraph  ─►  Candidates
                                                              │
                                          approved translations
                                                              ▼
       output  ◄─  ValidationReport  ◄─  built Archive  ◄─  patch + repack
```

Each stage writes its result into the project directory, so any stage can be re-run, inspected or
handed to a different person without re-running the ones before it.

## Modules

### `classfile` — the spine

Parses **only** the constant pool and keeps everything after it as an opaque `tail: Vec<u8>`,
copied byte for byte on write.

This is what makes the whole tool safe. Nothing outside the constant pool refers to a constant by
byte offset - only by index - so a Utf8 constant can grow or shrink freely and every other
structure in the class remains correct. A tool that parsed the whole class would have to get
`StackMapTable`, `BootstrapMethods` and every attribute right, and would break a class the moment
it met one it did not understand.

Strings are **modified UTF-8** (JVMS 4.4.7), not UTF-8: NUL is `C0 80`, and characters outside the
BMP are written as an encoded surrogate pair rather than a four-byte sequence. A standard UTF-8
reader rejects perfectly valid class files, so `decode_modified_utf8` and `encode_modified_utf8`
are separate from anything in the standard library.

Translatable text is the set of Utf8 constants referenced by a `CONSTANT_String`. That excludes
class names, field names and descriptors, which are Utf8 too and are what a naive
"replace the strings" tool destroys.

### `jar` — archives and manifests

Reads and writes JARs, and parses the `Key: value` format shared by JAR manifests and JAD
descriptors, including the 72-byte continuation-line wrapping (broken only at character
boundaries, so a multi-byte character is never split across lines).

Every archive is untrusted input. Entry paths that escape the output directory are rejected via
`enclosed_name`; entry sizes are checked against the declared size *and* the real read length, in
case the header lied; total size and entry count are capped.

Writing uses fixed 1980 timestamps and preserves entry order, so building the same inputs twice
gives byte-identical output. A build that embeds the current time cannot be diffed or verified by
hash.

### `encoding` — what charset is this actually in

J2ME games predate any consistent convention, so a declared charset is a hint at best. Candidate
encodings are scored on how plausible the decoded text is - replacement characters and control
characters count heavily against - and the winner comes with a confidence value that the caller
can act on.

### `detect` — capabilities, not identities

Produces a `CapabilityManifest`: what this archive *can do* (`cldc11`, `midlet_entry`,
`constant_pool_text`, `bitmap_font_candidates`, `obfuscated_names`, ...), each with a confidence
and the evidence behind it.

This is the module that keeps game specifics out of everything downstream. Downstream code asks
"does this game keep its text in the constant pool?", never "is this game X?".

### `graph` — what text is in here

Extraction into `TextNode`s, each with a stable id derived from its **location plus its original
text**. Re-analysing an unchanged game produces the same ids, so approved translations survive a
re-import; if the text at a location changes, so does the id, and a stale translation is not
silently reused against text it was never written for.

Classification is by shape - placeholders, punctuation, casing, slashes - never by game. Two rules
earn their place:

- Anything containing a slash but no spaces is technical. `com/example/Main` and `/img/hud.png`
  are class names and resource paths; display text with a slash, like `HP: 10 / 20`, has spaces
  around it. Without this rule a class name gets offered for translation and translating it
  silently breaks every class that references it.
- Archive metadata - `META-INF/`, `.MF`, `.JAD`, `.SF` - is never game text. It decodes fine, so a
  plain "is this text?" check happily offers `MIDlet-1: Game,/icon.png,Main` for translation, and
  translating it renames the entry point.

### `vietnamese` — glossary, memory, quality

Glossary terms (optionally locked), a translation memory with exact and fuzzy lookup, and quality
checks for lost placeholders, spacing and length growth.

### `suggest` — candidates

Proposes translations from the memory and glossary. Auto-approval is deliberately narrow: an
exact memory hit or a locked glossary term, both of which restate an existing decision. Anything
inferred stays a proposal.

### `build` — patch and repack

Groups patches by class so each class is parsed and re-serialised once however many of its
literals changed. Branding goes into separate `META-INF` files; the original manifest is not
touched.

### `validate` — is this still a game

`validate` compares a build against its original: nothing lost, every class still parses, entry
points still exist, placeholders survived, original manifest attributes preserved.

`inspect` is the weaker check for a JAR handed over without its project: well formed, classes
parse, entry point exists, text decodes. It cannot tell whether anything was lost. The two are
separate functions rather than one pretending to do both.

### `project` — the on-disk workspace

A directory of reviewable files, not a database. The original is hashed on import and re-hashed on
open; project.json carries a schema version and a revision; each build is recorded with the source
hash it came from and keeps its own output, so a bad localization can be rolled back without
losing the build that replaced it.

Output is written under `builds/` first and copied to `output/` second, so `output/` only ever
holds a build that finished and has a record.

### `tjlocalizer-desktop` and `desktop/` — the application

A Tauri shell: a Rust backend calling the core directly, and a React and TypeScript interface.
No localization logic is on the TypeScript side. The rule that makes this concrete is
auto-approval - whether a candidate may be taken without a human is decided by `suggest::apply_safe`
in Rust, where the tests are, and the interface only renders the answer.

The view models in `state.rs` exist because the core's types are shaped for correctness rather
than for a table: a `TextNode` knows nothing about its translation and vice versa, while a row of
the interface needs both. Joining them in Rust keeps that joining testable.

See [DESKTOP.md](DESKTOP.md).
