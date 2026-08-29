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

Or in one pass:

```sh
tjlocalizer localize game.jar --target vi-VN --style natural-dialogue
```

A one-shot run over a game with no translation memory behind it approves almost nothing and
produces a working but largely untranslated archive. That is the honest outcome, and the CLI says
so rather than reporting success.

## What it will not do

- **Translate on its own authority.** Candidates come from the project's own memory and glossary.
  An exact memory hit or a locked glossary term may be approved automatically, because both
  restate a decision the project already made. A fuzzy match never is: `Mở khóa` and `Mở khoá`
  score as near-identical and one of them is wrong.
- **Claim ownership of the game.** Attribution covers the localization only; the original
  manifest attributes are preserved and validation fails the build if any were changed or
  removed.
- **Trust the archive.** Every JAR is untrusted input: path traversal, oversized entries and
  entry-count bombs are refused before anything is allocated.

## Layout

| Crate | What it holds |
| --- | --- |
| `crates/tjlocalizer-core` | Everything: class files, archives, detection, extraction, translation, build, validation |
| `crates/tjlocalizer-cli` | `tjlocalizer`, one subcommand per workflow step |

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the pieces fit,
[docs/SPEC-COVERAGE.md](docs/SPEC-COVERAGE.md) for what is built and what is not, and
[docs/LEGAL.md](docs/LEGAL.md) for the attribution boundary.

## Status

The pipeline works end to end and is verified against a real JVM: the test suite localizes a
`javac`-produced class with Vietnamese text longer than the original, and the JVM loads, verifies
and runs the result. Several large parts of the specification are not built yet -
`docs/SPEC-COVERAGE.md` lists them plainly rather than leaving them to be discovered.

Localization by Thanhtinz. © 2026 Thanhtinz.
