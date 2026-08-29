# Specification coverage

Against *Thanhtinz JAR Localizer Professional Spec v2*. Sections are listed whether or not they
are built, because a reader needs to know what is missing as much as what is there.

| § | Section | State | Notes |
| --- | --- | --- | --- |
| 1 | Product vision | n/a | |
| 2 | No-hardcode architecture | **Built** | Enforced throughout; no game name, class name or resource path appears in the core. |
| 3 | Genre and game scope | **Partial** | Anything whose text is in the constant pool or in text resources. Genre-specific handling would come from plugins (§20), which are not built. |
| 4 | System architecture | **Built** | The pipeline in `docs/ARCHITECTURE.md`. |
| 5 | Generic game model | **Partial** | `CapabilityManifest` and `ContentGraph` exist; the dialogue and character layers do not. |
| 6 | Capability detection | **Built** | `detect`: platform, text storage, resources, obfuscation, each with confidence and evidence. |
| 7 | JAR/JAD analysis | **Built** | `jar`, including manifest and JAD parsing and 72-byte line wrapping. |
| 8 | Resource and text extraction | **Built** | `graph`, with stable node ids and shape-based classification. |
| 9 | Encoding and charset | **Built** | `encoding` (confidence-scored detection) plus modified UTF-8 in `classfile`. |
| 10 | Context intelligence | **Partial** | Classification by shape into UI, dialogue, format, technical and so on. No cross-node inference. |
| 11 | Vietnamese language engine | **Partial** | Quality checks for placeholders, spacing and length. No register or grammar model. |
| 12 | Vietnamese dictionary | **Not built** | The project reserves `dictionary/`; nothing populates or reads it. |
| 13 | Glossary and translation memory | **Built** | `vietnamese`: locked terms, exact and fuzzy memory, `suggest` for candidates. |
| 14 | Natural dialogue engine | **Not built** | `styleProfile` is recorded in project.json and otherwise unused. |
| 15 | Character and relationship model | **Not built** | |
| 16 | Font engine | **Not built** | Vietnamese glyph generation for bitmap fonts. `bitmap_font_candidates` detection exists; nothing acts on it. |
| 17 | Asset and OCR pipeline | **Not built** | |
| 18 | Bytecode analysis and patching | **Partial** | Constant-pool rewriting is complete and JVM-verified. Semantic patching beyond the pool is not built. |
| 19 | Rule engine | **Not built** | The project reserves `rules/`. |
| 20 | Plugin and adapter SDK | **Not built** | The design leaves room for it - detection is already decoupled from action - but no plugin boundary exists. |
| 21 | Project system | **Built** | `project`: immutable original, versioned profile, recorded builds, rollback. |
| 22 | Localization workflow | **Partial** | Steps 1-8, 11 and 14-17 are built. Dialogue processing, font generation, asset OCR and rules are not. |
| 23 | Build and repackaging | **Built** | Deterministic output, manifest preservation, SHA-256, build record. |
| 24 | Validation and QA | **Partial** | Structural, class, resource, encoding, entry point, placeholder and attribution checks. Glyph, layout, asset and terminology checks are not built. |
| 25 | Emulator and visual regression | **Not built** | `tjlocalizer test` performs static checks only and says so. |
| 26 | Branding and attribution | **Built** | See `docs/LEGAL.md`. |
| 27 | CLI and automation | **Built** | Every subcommand in the specification, plus `builds` and `rollback`. The same pipeline is driveable from the desktop application. |
| 28 | Data model | **Partial** | project.json matches §28.1. The entities for dialogue, fonts, assets, rules and patches are not built. |
| 29 | Security | **Built** | Untrusted-input handling and archive limits; no extracted code is executed. |
| 30 | Technology stack | **Built** | Rust core and CLI, Tauri desktop shell with a React and TypeScript interface. |
| 31 | Source structure | **Built** | Three crates (core, CLI, desktop) plus the interface under `desktop/`. |
| 32 | API and extension contracts | **Not built** | Depends on §20. |
| 33 | Performance and scalability | **Partial** | Patches are grouped per class so each is parsed once; nothing has been profiled at scale. |
| 34 | Roadmap | n/a | |
| 35 | Definition of done | **Partial** | The P0 row - JAR/JAD, project, analyzer, build and repack - is done and JVM-verified. |

## What "built" means here

The pipeline is exercised end to end by the test suite against a `javac`-produced fixture, and
`tools/verify-roundtrip.sh` goes further: it patches a class with Vietnamese text longer than the
original and runs it on a real JVM, which loads and verifies it. `tools/verify-desktop.sh` boots
the real desktop binary against a virtual display and checks that it opens and renders. That is
the standard used above.
