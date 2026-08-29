# The desktop application

A Tauri application: a Rust backend that calls `tjlocalizer-core` directly, and a React and
TypeScript interface (specification §30).

```
crates/tjlocalizer-desktop/   Rust: the commands, the view models, the window
desktop/                      TypeScript: the interface
```

## Building and running

```sh
# system dependencies (Debian/Ubuntu)
sudo apt-get install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev

npm --prefix desktop install
npm --prefix desktop run build      # typechecks, then builds the interface
cargo run --release -p tjlocalizer-desktop
```

The release binary embeds the built interface, so it runs on its own with no server. For a
hot-reload loop, run the dev server and turn the embedding off:

```sh
npm --prefix desktop run dev                              # terminal 1
cargo run -p tjlocalizer-desktop --no-default-features    # terminal 2
```

That switch is a Tauri feature (`custom-protocol`), not a build profile. Upstream leaves it off
and lets the Tauri CLI add it; this project builds with plain cargo, so it is on by default -
otherwise `cargo build --release` would quietly produce a binary that needs a dev server running.

## What lives where

**No localization logic on the TypeScript side.** The interface decides what to show; the core
decides what is true. The clearest case is auto-approval: whether a candidate may be taken without
a human is answered by `suggest::apply_safe` in Rust, where the tests are, and the interface only
renders the answer. A checkbox in TypeScript could not be trusted with that.

`state.rs` holds the view models. They exist because the core's types are shaped for correctness
rather than for a table - a `TextNode` knows nothing about its translation, and a translation
knows nothing about its node - and the interface needs both together, per row. Joining them in
Rust keeps the joining testable; `tests/view_models.rs` covers the cases where a display bug would
become a correctness bug.

`commands.rs` is a thin wrapper over the core, one command per action.

## Outside the desktop shell

Opened in a plain browser, the interface renders a short note saying it needs the desktop shell,
and nothing else. There is deliberately no stand-in data: a screenshot of the interface must never
be mistakable for a screenshot of it working.

## The three screens

**Tổng quan** - the pipeline in the order §22 runs it, with each step's state visible rather than
inferred, the detected capabilities with their evidence, the project's facts (the original's hash,
the profile revision), and the settings that go into project.json.

**Văn bản** - the translation table. Filters by context, status and free text; a detail panel with
the original, where it lives, the placeholders that must survive, the current candidate with its
origin, and the quality warnings, recomputed on every read so a stale green row is impossible.
Non-translatable strings are hidden by default rather than dropped, so "where did that string go?"
has an answer.

**Đóng gói** - build, the validation report, and the build history with rollback.

## Checking it

`tools/verify-desktop.sh` boots the real release binary against a virtual display and checks that
a window appears and that the interface rendered. Compiling proves none of that: a missing system
library, a bad config, or assets that were not embedded all produce a binary that builds and then
shows an error page. The script measures the window's mean brightness, because that error page is
almost white and the interface is not - a broken build measures 0.999, a working one 0.10.
