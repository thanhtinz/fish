# TJLocalizer

A localization platform for Java and J2ME game archives, targeting Vietnamese (vi-VN).

It reads a JAR, works out what the game supports, extracts the text that is genuinely game
content, applies approved translations, repackages, and refuses to call the result good unless it
still holds together.

## The one rule that shapes everything

**Nothing in the core knows which game it is looking at.** There are no game names, class names,
resource paths, screen coordinates or string ids anywhere in `tjlocalizer-core`. What a game
supports is *detected*; what is done about it comes from the project's capabilities, glossary,
memory and rules.

```rust
// never
if game == "SomeGame" { patch_class("com/some/Menu"); }

// always
let capabilities = detect::detect(&archive);
let graph = graph::extract(&archive);
```

The practical test: every rule in the codebase is about a *format* - the JAR layout, the class
file format, the shape of a format string - not about a title. Where that line was hard to hold,
the reason is written down at the code.

## Quick start

### The desktop application

```sh
# system dependencies (Debian/Ubuntu)
sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev

npm --prefix desktop install && npm --prefix desktop run build
cargo run --release -p tjlocalizer-desktop
```

Import a JAR, work through the pipeline, translate, build. See
[docs/DESKTOP.md](docs/DESKTOP.md).

### The command line

```sh
cargo build --release
./tools/make-fixtures.sh              # needs a JDK; builds the test fixtures

tjlocalizer import game.jar --into projects/game
tjlocalizer analyze  projects/game    # what does this game support?
tjlocalizer extract  projects/game    # what text is in it?
tjlocalizer translate projects/game --apply-safe
#   ... a human translates the rest, in projects/game/translations/ ...
tjlocalizer build    projects/game
tjlocalizer test     projects/game/output/game-vi-vn.jar
```

Several languages at once:

```sh
tjlocalizer targets projects/game --add th
tjlocalizer translate projects/game --all --apply-safe
tjlocalizer build projects/game --all
tjlocalizer export projects/game ~/Desktop --all
```

A PC game installed on disk is a folder, not a file, and that works too:

```sh
tjlocalizer import ~/.steam/steamapps/common/FishingGame --into projects/fishing
#   20 006 files (12.1 MB), read 3 (164 B)
tjlocalizer extract projects/fishing
#   ... translate ...
tjlocalizer build projects/fishing                    # writes a patch: only what changed
tjlocalizer apply-patch projects/fishing --to ~/.steam/steamapps/common/FishingGame
```

The tree is walked without opening anything; only files in a format this build reads are copied in.
`build` never writes into the game — applying is a separate command that checks every file against
the version the patch was built from, keeps what it replaced, and refuses the whole patch rather
than writing part of one.

Reading the words painted into artwork with the game's own letters, being told what a game is, and
looking at what a build changed:

```sh
tjlocalizer assets  projects/game --read --accept   # only where every shape matched
tjlocalizer context projects/game --cast            # who the game names, and where
tjlocalizer plugins projects/game                   # adapters, and what each claims here
tjlocalizer regress projects/game --accept          # this is what it should look like
tjlocalizer regress projects/game                   # what changed since?
tjlocalizer play    projects/game --command emulator
```

Or in one pass:

```sh
tjlocalizer localize game.jar --target vi-VN,en,th --style natural-dialogue
```

A one-shot run over a game with no translation memory behind it approves almost nothing and
produces a working but largely untranslated archive. That is the honest outcome, and the CLI says
so rather than reporting success.

Asking Claude which files to look at, what an unknown file is, and what looks wrong with the
translation - all off until switched on, and all of it suggestions rather than findings:

```sh
tjlocalizer claude  projects/game --key - --enable
tjlocalizer analyze projects/game --with-claude    # sends file names, never contents
tjlocalizer inspect projects/game assets/data.bin --dry-run
tjlocalizer review  projects/game --lang vi-VN --dry-run
```

A scan sends file names, sizes and what the mechanical check already made of each. Asking about one
file sends the first 2 KiB of that one file. `--dry-run` prints the whole request and sends
nothing. See [docs/LANGUAGES.md](docs/LANGUAGES.md).

## What it will not do

- **Translate on its own authority.** Candidates come from the project's own memory, glossary and
  dictionaries. An exact memory hit or a locked glossary term may be approved automatically,
  because both restate a decision the project already made. A fuzzy match never is: `Mở khóa` and
  `Mở khoá` score as near-identical and one of them is wrong. A dictionary gloss never is either,
  however complete - see [docs/LANGUAGES.md](docs/LANGUAGES.md).
- **Claim ownership of the game.** Attribution covers the localization only; the original
  manifest attributes are preserved and validation fails the build if any were changed or
  removed.
- **Trust the archive.** Every JAR is untrusted input: path traversal, oversized entries and
  entry-count bombs are refused before anything is allocated.

## Layout

| Directory | What it holds |
| --- | --- |
| `crates/tjlocalizer-core` | Everything that decides anything: class files, archives, detection, extraction, translation, build, validation |
| `crates/tjlocalizer-cli` | `tjlocalizer`, one subcommand per workflow step |
| `crates/tjlocalizer-desktop` | The desktop application's Rust side: commands and view models |
| `desktop` | The desktop interface, React and TypeScript |

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the pieces fit,
[docs/DESKTOP.md](docs/DESKTOP.md) for the application,
[docs/LANGUAGES.md](docs/LANGUAGES.md) for dictionaries and register,
[docs/FONTS.md](docs/FONTS.md) for the glyph engine,
[docs/RULES.md](docs/RULES.md) for the per-game patches,
[docs/PLUGINS.md](docs/PLUGINS.md) for adapters written as data,
[docs/CONTEXT.md](docs/CONTEXT.md) for what a line is for and who says it,
[docs/REGRESSION.md](docs/REGRESSION.md) for comparing drawings and running builds,
[docs/PERFORMANCE.md](docs/PERFORMANCE.md) for what it costs to run,
[docs/PACKAGES.md](docs/PACKAGES.md) for the package kinds and text formats,
[docs/ASSETS.md](docs/ASSETS.md) for words painted into artwork,
[docs/SPEC-COVERAGE.md](docs/SPEC-COVERAGE.md) for what is built and what is not, and
[docs/LEGAL.md](docs/LEGAL.md) for the attribution boundary.

## Status

The pipeline works end to end and is verified against a real JVM: the test suite localizes a
`javac`-produced class with Vietnamese text longer than the original, and the JVM loads, verifies
and runs the result - including a class whose *code* was patched, where a mistake about
instruction lengths would produce something the verifier rejects rather than a wrong string. The
desktop application is checked the same way: a script boots the real binary and confirms it opens
and renders, because compiling proves neither.

Every section of the specification is now built. That is a statement about the specification
rather than a claim that the tool does everything: several parts are built around a deliberate
refusal - no general OCR, no emulator, no plugin that runs, no bytecode that grows, no register
chosen automatically - and `docs/SPEC-COVERAGE.md` collects those refusals at the bottom, with the
reason for each.

Localization by Thanhtinz. © 2026 Thanhtinz.
