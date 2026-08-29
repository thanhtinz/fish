# Specification coverage

Against *Thanhtinz JAR Localizer Professional Spec v2*. Sections are listed whether or not they
are built, because a reader needs to know what is missing as much as what is there.

| § | Section | State | Notes |
| --- | --- | --- | --- |
| 1 | Product vision | n/a | |
| 2 | No-hardcode architecture | **Built** | Enforced throughout; no game name, class name or resource path appears in the core. |
| 3 | Genre and game scope | **Partial** | Anything whose text is in a Java constant pool or in a text resource this build reads - which now covers Android, iOS and a good deal of what PC games ship. Genre register is handled by style profiles; genre-specific *code* handling would come from plugins (§20), which are not built. |
| 4 | System architecture | **Built** | The pipeline in `docs/ARCHITECTURE.md`. |
| 5 | Generic game model | **Partial** | `CapabilityManifest`, `ContentGraph`, language and register model. The character and relationship layer is typed but not populated. |
| 6 | Capability detection | **Built** | `detect`: platform, text storage, resources, obfuscation, each with confidence and evidence. |
| 7 | Package analysis | **Built** | `jar` for the archive itself, including manifest and JAD parsing and 72-byte line wrapping; `package` identifies J2ME MIDlets, Java archives, Android APKs, iOS IPAs and plain zips from their contents, and says which can be rebuilt here and which need a signature this tool has no business holding. See `docs/PACKAGES.md`. |
| 8 | Resource and text extraction | **Built** | `graph`, with stable node ids and shape-based classification, over `resource`: properties, Android `strings.xml`, Apple `.strings`, gettext `.po` and JSON, each written back in place so comments and formatting survive, plus INI with section-qualified keys, Unreal's binary `.locres`, and Ren'Py's generated translation files - where the game's own script is deliberately read-only, because rewriting it is not how Ren'Py is localized. A game installed as a directory is scanned without opening anything, read selectively, and built back as a patch of only the files that changed. |
| 9 | Encoding and charset | **Built** | `encoding` (confidence-scored detection) plus modified UTF-8 in `classfile`. |
| 10 | Context intelligence | **Partial** | Classification by shape, and dictionary domains matched against it. No cross-node inference. |
| 11 | Vietnamese language engine | **Built** | Register profiles with pronoun sets, register-break detection, missing-diacritic and unconverted-Telex/VNI checks, plus the language-general quality checks. |
| 12 | Dictionary | **Built** | 633 game-domain entries across eight directions, embedded; projects may add their own packs. See `docs/LANGUAGES.md`. |
| 13 | Glossary and translation memory | **Built** | `vietnamese`: locked terms, exact and fuzzy memory, `suggest` for candidates. |
| 14 | Natural dialogue engine | **Partial** | Register profiles decide the voice, are sent to an external engine as instructions, and are checked against every reply. Sentence-level generation is the external engine's, and its output is never auto-approved - see `docs/LANGUAGES.md`. |
| 15 | Character and relationship model | **Partial** | `Speaker` and `Stance` select the voice; nothing yet infers them per line. |
| 16 | Font engine | **Partial** | Glyph sheets read, coverage reported, missing-glyph scan wired into validation, the 134 Vietnamese letters composed from the game's own letters, and the composed sheet installed into the archive by a rule (§19). Teaching a game to read the new rows is per-game and is left to a hand-written rule - see `docs/FONTS.md` and `docs/RULES.md`. |
| 17 | Asset and OCR pipeline | **Partial** | No OCR: nothing reads the words, and a wrong reading would be worse than none. Every image is inventoried with structured evidence about whether it looks like a label, a person marks the ones that carry words, and from then on the build reports each marked image that still ships its original artwork - including one that was redrawn but never installed. See `docs/ASSETS.md`. |
| 18 | Bytecode analysis and patching | **Partial** | Constant-pool rewriting is complete and JVM-verified, including integer constants, and reachable from rules (§19). Semantic patching beyond the pool is not built and rules cannot express it. |
| 19 | Rule engine | **Built** | Rules are data in `rules/rules.json`: conditions checked against the actual archive, three actions (replace an entry, change an int or string constant in a named class), off until switched on, planned before they run and recorded in the build. Deliberately cannot add bytecode. See `docs/RULES.md`. |
| 20 | Plugin and adapter SDK | **Not built** | The design leaves room for it - detection is already decoupled from action - but no plugin boundary exists. |
| 21 | Project system | **Built** | `project`: immutable original, versioned profile, recorded builds, rollback. |
| 22 | Localization workflow | **Partial** | Steps 1-11 and 14-17 are built, plus font generation, rules, and an asset inventory that tracks artwork carrying words. Reading those words needs OCR, which is not built. |
| 23 | Build and repackaging | **Built** | Deterministic output, manifest preservation, SHA-256, build record. |
| 24 | Validation and QA | **Partial** | Structural, class, resource, encoding, entry point, placeholder, attribution, per-language length and script, terminology, register, **glyph** and **layout width** checks - the last measured in the game's own pixels from its glyph sheet, for interface text on proportional fonts. Where the layout check fires, `shorten` offers narrower wordings from the project's own dictionary and interface register, measured and never applied. Marked artwork that still ships its original words is reported (§17), and approved translations aimed at a file this build cannot write are reported per file rather than vanishing. |
| 25 | Emulator and visual regression | **Partial** | No emulator: nothing here runs the game. `tjlocalizer proof` draws every approved translation with the game's own glyphs, at its own size, with a marker where the original ended, and the Text tab draws the selected row the same way. That catches what a report cannot - a mark landing on the letter below it, a stack that smudges at twelve pixels - and catches nothing about menus, backgrounds or timing. `tjlocalizer test` is still static. |
| 26 | Branding and attribution | **Built** | See `docs/LEGAL.md`. |
| 27 | CLI and automation | **Built** | Every subcommand in the specification, plus `builds` and `rollback`. The same pipeline is driveable from the desktop application. |
| 28 | Data model | **Partial** | project.json is schema 3: a source language and a list of targets, migrated from schema 2 on open. DictionaryStore, a register model, a font profile and a rule set exist; assets do not. |
| 29 | Security | **Built** | Untrusted-input handling and archive limits; no extracted code is executed; the network engine is off by default and its key is stored outside the project in an owner-only file. |
| 30 | Technology stack | **Built** | Rust core and CLI, Tauri desktop shell with a React and TypeScript interface, native file and save dialogs. |
| 31 | Source structure | **Built** | Three crates (core, CLI, desktop) plus the interface under `desktop/`. |
| 32 | API and extension contracts | **Partial** | `translate::Provider` is a stable seam, with an HTTP implementation covering four API families. The plugin boundary of §20 is not built. |
| 33 | Performance and scalability | **Partial** | Patches are grouped per class so each is parsed once; nothing has been profiled at scale. |
| 34 | Roadmap | n/a | |
| 35 | Definition of done | **Partial** | The P0 row - JAR/JAD, project, analyzer, build and repack - is done and JVM-verified. |

## What "built" means here

The pipeline is exercised end to end by the test suite against a `javac`-produced fixture, and
`tools/verify-roundtrip.sh` goes further: it patches a class with Vietnamese text longer than the
original and runs it on a real JVM, which loads and verifies it. `tools/verify-desktop.sh` boots
the real desktop binary against a virtual display and checks that it opens and renders. That is
the standard used above.

Where a row says **Built** for something involving language, read `docs/LANGUAGES.md` first: it
states plainly what the dictionary and register layers do and do not do, and the boundary matters
more than the checkmark.
