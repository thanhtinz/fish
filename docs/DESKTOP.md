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

## The five screens

**Tổng quan** - the pipeline in the order §22 runs it, with each step's state visible rather than
inferred; the target languages and their progress; the register picker, which shows what each
profile does to the pronouns; the dictionary directions actually usable for this project's
language pair, said plainly when there are none; the detected capabilities with their evidence;
and the project's facts. Below those: the cast the game names and how many lines were read from
their surroundings rather than from their own shape (§10, §15), and the adapters this project
carries with what each one claims and whether any of it matched this game (§20) - both shown only
when there is something to show.

A banner stays up while the source language is still a guess. A wrong one silently disables every
dictionary, so it is worth interrupting for.

**Văn bản** - the translation table, for the language selected in the title bar. Filters by context, status and free text; a detail panel with
the original, where it lives, the placeholders that must survive, the current candidate with its
origin, and the quality warnings, recomputed on every read so a stale green row is impossible.
Non-translatable strings are hidden by default rather than dropped, so "where did that string go?"
has an answer.

**Font** - the game's own font, which is the difference between a correct translation and a
correct translation that displays. Most J2ME games draw text from an image holding the letters
they were written for, and a game from China, Japan or Korea holds ASCII and nothing else, so
Vietnamese renders as blanks until somebody deals with it.

The tab does four things, and offers rather than decides at every one of them:

- **Which image is the font.** Every PNG in the archive is ranked by how much it looks like a
  glyph sheet - few colours, little ink, letters sitting inside a grid - and shown with a
  thumbnail, because what separates a glyph sheet from a sprite atlas is often obvious only to
  somebody who has seen the game run.
- **Which grid it uses.** The grids that divide the image evenly are scored on three counts and
  offered in order. The tool never picks: a grid off by a pixel shifts every glyph and reads as a
  rendering bug rather than as a wrong setting.
- **Where the marks come from.** Drawn from pixels by default, or borrowed from a folder of
  typefaces on the user's own disk. Nothing is copied into the project - the path is remembered
  and the font is read from where its owner keeps it - so a project can still be sent to a
  translator. Fonts are measured against this game's sheet rather than read off the file, because
  at twelve pixels no property of a font says how many of its marks will survive.
- **What it looks like.** The sample text renders with both, at the size that ships, because
  which reads better is not a thing a count can answer.

Composing writes the extended sheet into the project's `fonts/`. Installing it is a rule, and the
rule replaces the image - the same in every game. Telling the game the sheet grew taller is not,
and the tab lists what *looks* like where the game records that: a class holding the sheet's row
count, a string listing its characters in order. Found, not verified, and it says so where the
button is rather than in a footnote.

Coverage is reported in three states, not two: covered, missing, and *nobody has said*. A project
whose font has never been declared has an unknown answer rather than a good one, and showing the
two the same way is how a localization ships with empty boxes where the accents should be.

The Text tab also draws the selected row with the game's own glyphs, above the two widths, so a
mark landing on the letter below it is visible while translating rather than after building.

**Ảnh** - the images in the game, with what each one's shape suggests about words painted into it.
Where the project knows the game's glyph sheet, **Đọc chữ bằng font game** reads the words straight
off each picture by matching it against those same letters, fills in the ones where every shape
matched, and says how many shapes matched nothing on the ones where they did not. Nothing is saved
until a person presses the button. From then on every build reports each marked image that still
carries its original words, which is the only way that blind spot stays visible: no string check
can see it, because the word was never a string.

**Đóng gói** - build one language or all of them, the validation report, the build history with
rollback, and **Xuất file ra…**, which opens a native save dialog. Where the finished file goes is
the user's business, not the tool's; the project directory is the tool's.

Underneath: the drawing of every approved translation compared against the one somebody accepted,
with what moved marked in red over it (§25). Six lines edited and sixty changed means something
else moved - a font recomposed, a sheet's letters sitting a pixel lower - and no text report shows
that.

## Files in and out

Everything a desktop application should let you choose a path for:

| | |
| --- | --- |
| Nhập file JAR… | pick the game, then pick where the project lives |
| Xuất file ra… | save the built JAR anywhere, under any name |
| Xuất CSV… | every string, for a translator working outside the app |
| Nhập CSV… | read the translator's file back, matched by node id |
| Nhập gói từ điển… | add a dictionary pack to the project |

## The external engine

A card in **Tổng quan** configures it, and it is off until someone turns it on. The card is
written to be read before it is used: turning it on sends the game's text to somebody else's
computer, which is a decision, and the interface's job is to make it one rather than a checkbox
nobody read. It shows a red badge while enabled, states that the key is stored outside the project
and why, and **Xem thử request** prints the exact request - with the key replaced by a placeholder,
so a screenshot of it is safe.

In **Văn bản**, the engine is asked one string at a time and only when the button is pressed. It
appears only when an engine is configured, enabled and has a key, so pressing it can never reach
the network by surprise. The reply arrives with whatever the checks found: a lost placeholder is a
refusal, a broken register or an ignored glossary term is a warning beside the text.

The CSV carries a byte-order mark, because Excel reads a CSV without one as the system's legacy
encoding - which turns every Vietnamese diacritic and every Thai character into rubbish, and the
translator's tool is Excel. Its quoting is written out and tested rather than pulled in: game text
is full of commas and quotes, and a field that lands in the wrong column is approved as the
translation of the wrong string, which nobody notices until the game ships.

Import matches by node id, so a row whose source text was edited still lands in the right place,
and a row for a string that no longer exists is reported rather than silently dropped.

## Checking it

`tools/verify-desktop.sh` boots the real release binary against a virtual display and checks that
a window appears and that the interface rendered. Compiling proves none of that: a missing system
library, a bad config, or assets that were not embedded all produce a binary that builds and then
shows an error page. The script measures the window's mean brightness, because that error page is
almost white and the interface is not - a broken build measures 0.999, a working one 0.10.
