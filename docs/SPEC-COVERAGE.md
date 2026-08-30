# Specification coverage

Against *Thanhtinz JAR Localizer Professional Spec v2*. Sections are listed whether or not they
are built, because a reader needs to know what is missing as much as what is there.

Every section is now built. That is a statement about the specification, not a claim that nothing
is left: several rows are built *around a deliberate refusal*, and those refusals are the most
important thing on this page. They are collected at the bottom.

| § | Section | State | Notes |
| --- | --- | --- | --- |
| 1 | Product vision | n/a | |
| 2 | No-hardcode architecture | **Built** | Enforced throughout; no game name, class name or resource path appears in the core. |
| 3 | Genre and game scope | **Built** | Anything whose text is in a Java constant pool or in a text resource this build reads - which covers Android, iOS and a good deal of what PC games ship - plus whatever a plugin names as a resource of a format this build writes (§20). Genre register is handled by style profiles. |
| 4 | System architecture | **Built** | The pipeline in `docs/ARCHITECTURE.md`. |
| 5 | Generic game model | **Built** | `CapabilityManifest`, `ContentGraph`, language and register model, and a cast filled in from the lines themselves: who is named, how often, in which files, and beside whom. What is deliberately absent is a relationship graph asserting more than strings can support - co-occurrence is stated as co-occurrence. See `docs/CONTEXT.md`. |
| 6 | Capability detection | **Built** | `detect`: platform, text storage, resources, obfuscation, each with confidence and evidence. |
| 7 | Package analysis | **Built** | `jar` for the archive itself, including manifest and JAD parsing and 72-byte line wrapping; `package` identifies J2ME MIDlets, Java archives, Android APKs, iOS IPAs and plain zips from their contents, and says which can be rebuilt here and which need a signature this tool has no business holding. See `docs/PACKAGES.md`. |
| 8 | Resource and text extraction | **Built** | `graph`, with stable node ids and shape-based classification, over `resource`: properties, Android `strings.xml`, Apple `.strings`, gettext `.po` and JSON, each written back in place so comments and formatting survive, plus INI with section-qualified keys, Unreal's binary `.locres`, and Ren'Py's generated translation files - where the game's own script is deliberately read-only, because rewriting it is not how Ren'Py is localized. A game installed as a directory is scanned without opening anything, read selectively, and built back as a patch of only the files that changed. |
| 9 | Encoding and charset | **Built** | `encoding` (confidence-scored detection) plus modified UTF-8 in `classfile`. |
| 10 | Context intelligence | **Built** | Classification by shape, dictionary domains matched against it, and `context`: cross-node inference over key sections, constant-pool neighbours and named speakers, each reading carrying its evidence. It reconsiders only the reading a string got for being short, never one the string itself settled. See `docs/CONTEXT.md`. |
| 11 | Vietnamese language engine | **Built** | Register profiles with pronoun sets, register-break detection, missing-diacritic and unconverted-Telex/VNI checks, plus the language-general quality checks. |
| 12 | Dictionary | **Built** | 633 game-domain entries across eight directions, embedded; projects may add their own packs. See `docs/LANGUAGES.md`. |
| 13 | Glossary and translation memory | **Built** | `vietnamese`: locked terms, exact and fuzzy memory, `suggest` for candidates. |
| 14 | Natural dialogue engine | **Built** | Register profiles decide the voice, the speaker of each line is inferred and chooses the pronouns within it, both are sent to an external engine as instructions, and every reply is checked against them. Sentence-level generation is the external engine's, and its output is never auto-approved - see `docs/LANGUAGES.md`. |
| 15 | Character and relationship model | **Built** | `Speaker` and `Stance` select the voice, and `context` infers the speaker per line from how the game writes its speech, feeding it to every translation request. A character's stance is offered with the words behind it and never applied, because §14 leaves that decision to a person. |
| 16 | Font engine | **Built** | Both routes. **Extend the sheet:** glyph sheets read, coverage reported, missing-glyph scan wired into validation, the 134 letters composed from the game's own, the sheet installed by a rule, and the per-game half - telling the game the sheet grew - searched for and proposed rather than guessed. **Switch to the handset's font:** how a game draws its text is read from what its classes call, the methods that could stop blitting are found by shape, and a rule replaces one method's body with a call to `Graphics.drawString` - which is the route most J2ME localizations actually take, and the only one for a CJK-only sheet. Once such a rule is on, coverage becomes the handset's and the pixel-width check goes quiet, because the widths that now matter belong to a font this tool has never seen. See `docs/FONTS.md`. |
| 17 | Asset and OCR pipeline | **Built** | Every image is inventoried with structured evidence about whether it looks like a label, and where the project knows the game's glyph sheet the words are read straight off the picture by matching each shape against the game's own letters - accepted only where every shape matched, and reported as unread otherwise rather than guessed. A person marks the images that carry words, and from then on the build reports each marked image that still ships its original artwork - including one that was redrawn but never installed. Reading artwork lettered outside the game's font is not attempted, on purpose. See `docs/ASSETS.md`. |
| 18 | Bytecode analysis and patching | **Built** | Constant-pool rewriting, complete and JVM-verified, including integer constants. Method references are resolved, so what a class *does* can be asked without running it. Beyond the pool: each method's code is walked instruction by instruction, every place a string is loaded is found, and one such place can be pointed at a different constant - the case a pool rewrite cannot express, where one of eleven uses of `Back` needs different words. Operands only, never lengths, so jumps and stack maps are untouched, and a constant an `ldc` cannot reach is refused rather than widened. Reachable from rules as `setStringAtSite`, and verified by `tools/verify-roundtrip.sh` running the patched class on a real JVM. |
| 19 | Rule engine | **Built** | Rules are data in `rules/rules.json`: conditions checked against the actual archive, four actions (replace an entry, change an int or string constant in a named class, change what one named method loads), off until switched on, planned before they run and recorded in the build. Deliberately cannot add bytecode. See `docs/RULES.md`. |
| 20 | Plugin and adapter SDK | **Built** | `plugin`: adapters as JSON in the project's `plugins/`, contributing capabilities with evidence, resource formats, font hints, rules and dictionary packs. Data and only data - nothing is loaded or executed, because a plugin arrives by the same route as the untrusted archive (§29). See `docs/PLUGINS.md`. |
| 21 | Project system | **Built** | `project`: immutable original, versioned profile, recorded builds, rollback. |
| 22 | Localization workflow | **Built** | Every step, including font generation, rules, and an asset inventory that both tracks artwork carrying words and reads them where the game's own font drew them. |
| 23 | Build and repackaging | **Built** | Deterministic output, manifest preservation, SHA-256, build record. |
| 24 | Validation and QA | **Built** | Structural, class, resource, encoding, entry point, placeholder, attribution, per-language length and script, terminology, register, cross-game **consistency** (one label translated two ways, two labels translated the same way), **glyph** and **layout width** checks - the last measured in the game's own pixels from its glyph sheet, for interface text on proportional fonts. Where the layout check fires, `shorten` offers narrower wordings from the project's own dictionary and interface register, measured and never applied. Marked artwork that still ships its original words is reported (§17), and approved translations aimed at a file this build cannot write are reported per file rather than vanishing. |
| 25 | Emulator and visual regression | **Built** | No emulator is shipped, and none is guessed at: `tjlocalizer play` runs the command the project's owner wrote down, on the newest build, and nothing read out of a game can influence it (§29). Visual regression is real: `proof` draws every approved translation in the game's own glyphs at its own size, `regress --accept` keeps that drawing, and `regress` compares the next one pixel for pixel - reporting the rows that moved and writing the new picture with the changes marked. That catches what a text report cannot: six lines edited and sixty changed means a font was recomposed or a sheet's baseline moved. What it still says nothing about is menus, backgrounds and timing, which need the emulator a person runs. See `docs/REGRESSION.md`. |
| 26 | Branding and attribution | **Built** | See `docs/LEGAL.md`. |
| 27 | CLI and automation | **Built** | Every subcommand in the specification, plus `builds` and `rollback`. The same pipeline is driveable from the desktop application. |
| 28 | Data model | **Built** | project.json is schema 5, migrated from every earlier version on open and refused when newer than this build understands. It carries the source, a source language and a list of targets, branding, the engine and analyst settings, the font profile, the images marked as carrying words, and the emulator its owner runs. Beside it: dictionary packs, glossary and memory per direction, the content graph and its readings, rules, plugins and build records. |
| 29 | Security | **Built** | Untrusted-input handling and archive limits; no extracted code is executed; the network engine is off by default and its key is stored outside the project in an owner-only file. |
| 30 | Technology stack | **Built** | Rust core and CLI, Tauri desktop shell with a React and TypeScript interface, native file and save dialogs. |
| 31 | Source structure | **Built** | Three crates (core, CLI, desktop) plus the interface under `desktop/`. |
| 32 | API and extension contracts | **Built** | `translate::Provider` is a stable seam, with an HTTP implementation covering four API families, and the plugin boundary of §20 is a second: a declarative contract, versioned by what it may name, that cannot reach past what the core already does. |
| 33 | Performance and scalability | **Built** | Patches are grouped per class so each is parsed once, the dictionary is indexed rather than walked per question, and `tools/bench.sh` times every step a person waits through against a synthetic game of a quarter of a million strings. It scales linearly there, and the one step that did not - proposing translations, at 5.6 seconds for 40,000 lines - is now eighteen times faster. Memory at directory-game scale is not profiled and `docs/PERFORMANCE.md` says so. |
| 34 | Roadmap | n/a | |
| 35 | Definition of done | **Built** | Every row: JAR/JAD, project, analyzer, build and repack (P0, JVM-verified); resources, encodings, dictionary, register, validation and the desktop application (P1); fonts, assets, rules, plugins, bytecode sites, context and visual regression (P2). What each one does *not* do is stated in its own row above rather than left to be discovered. |

## What "built" means here

The pipeline is exercised end to end by the test suite against a `javac`-produced fixture, and
`tools/verify-roundtrip.sh` goes further: it patches a class with Vietnamese text longer than the
original and runs it on a real JVM, which loads and verifies it. `tools/verify-desktop.sh` boots
the real desktop binary against a virtual display and checks that it opens and renders. That is
the standard used above.

Where a row says **Built** for something involving language, read `docs/LANGUAGES.md` first: it
states plainly what the dictionary and register layers do and do not do, and the boundary matters
more than the checkmark.

## What is deliberately not here

Each of these could be added and each would make the tool worse. They are listed together so that
"every section is built" cannot be read as "it does everything".

**General OCR.** Words are read out of artwork by matching the picture against the game's own
glyph sheet, letter for letter, and a reading with one unmatched shape in it is not offered at
all. A model that returned `5TART` for a twelve-pixel button would produce text a translator has
to check against the picture anyway - and would be believed the one time nobody checks.

**An emulator.** Nothing here runs a game. `play` launches the emulator its owner already has,
from the command they wrote down; the drawings this tool compares are drawings of text, and they
say nothing about menus, backgrounds or timing.

**Plugins that run.** A plugin is data. It arrives beside the archive, by the same route, from the
same kind of stranger; a plugin format that could execute would make "open this game" mean "run
this program", and every guarantee in §29 would be worth nothing.

**Bytecode that grows.** One load instruction can be pointed at a different constant, and one
recognised font method can have its body *replaced* by a branchless call to the handset's font -
that one exception is what makes the second route to Vietnamese possible at all, and it is proved
by a real JVM's verifier rather than by a test. Nothing can add an instruction to code that
stays, and a constant an `ldc` cannot reach is refused rather than widened, because a method whose
jumps are one byte out fails verification in a way nobody can debug from a translated string.

**Automatic register.** A character's stance is offered with the words behind it and never
applied. Vietnamese has no neutral second person, so choosing between `ngươi` and `bạn` governs
every line that character speaks - and a decision inferred from the word "please" is not a
decision.

**Auto-approval of anything inferred.** An exact memory hit or a locked glossary term restates a
decision somebody already made. Everything else stays a proposal, including everything an external
engine returns.
