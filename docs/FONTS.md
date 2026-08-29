# The font engine

Specification §16. The problem it exists for, first, because it is not obvious:

A J2ME game that does not use the handset's own font draws its text from a PNG of glyphs laid out
on a grid. It can only show characters somebody drew into that sheet, and nobody drew `ế`. So a
localization can be **correct in every other way** — the translation is right, the placeholders
survived, the archive rebuilds, the classes verify, the game runs — and the screen shows blanks.
Nothing else in the pipeline can see it: by every other measure the text is fine.

Vietnamese is unusually exposed here. It needs **134 letters beyond ASCII**, because every vowel
takes a modifier, a tone, or both. A font that covers French, German or Spanish is nowhere near
enough.

## What it does

### Says whether the text will display

```sh
tjlocalizer font projects/game --sheet font.png --cell 8x12 --columns 16
```

```
95 glyphs in the sheet
134 of the 134 letters Vietnamese needs are missing, 134 of them composable
2 approved translations use 6 the font cannot draw: áòđơầắ
```

And once a font is declared, a build that would show blanks **fails validation**:

```
error font.glyph  "Bắt đầu trò chơi": the font has no glyph for ắđầòơ - this will show as blanks
```

A project with no font declared gets a **warning**, not silence. "Nobody established what this
game draws with" is a different answer from "it can draw everything", and a tool that conflates
them ships broken games.

### Draws the letters the game is missing

```sh
tjlocalizer font projects/game --compose
```

Each new glyph is built from **the game's own letter**: take its `e`, put a mark above, and the
result has the game's weight, its pixel grid and its personality. A glyph lifted from a real
typeface and set next to hand-drawn game text looks exactly like what it is.

Concretely:

- The base letter's pixels are copied unchanged, so a composed glyph *is* the game's letter.
- The ink colour is sampled from the game's own glyph, not assumed black — a font may be white,
  gold or outlined, and a mark in the wrong colour looks like a defect rather than an omission.
- Marks are positioned against the letter's **ink bounds**, not the cell. Cells are padded and the
  padding differs per glyph; a mark measured from the cell edge floats away from short letters.
- A tone stacks **above** a circumflex or breve rather than on top of it, or `ế` and `é` become
  the same picture — and they are different words.
- The original sheet is copied byte for byte and every original glyph keeps its index, so a game
  that indexes into the sheet by position still finds everything it had.

### Refuses rather than drawing badly

A clipped tone mark is a different word. When a letter has no clear rows above it (or below, for
the dot), the glyph is **skipped and reported** with the measurement:

```
skipped á - only 0 clear rows above the letter, 2 needed
```

## What it does not do

**It does not install the font.** Making the game *use* the new glyphs means changing how it looks
characters up, and every J2ME game does that differently — some index `char - 32`, some carry
their own table, some pack widths into a second file. That is a per-game patch and belongs to the
rule engine (§19), which is not built. This produces the artwork and a sidecar describing it:

```
fonts/extended.png     the sheet
fonts/extended.json    grid, glyph order, what was added, what was skipped
```

**It does not guess the grid.** `--cell` and `--columns` are required. A grid inferred from one
sheet is a guess about that sheet, and a wrong guess shifts every glyph by a pixel in a way that
reads as a rendering bug rather than a bad setting.

**It reads PNG only.** MIDP requires PNG support, so game sheets are PNG in practice.

## How the alphabet is defined

The 134 letters are **built from the twelve vowels** rather than written out, and each carries how
to compose it: a base ASCII letter, an optional vowel modification (breve, circumflex, horn,
stroke) and an optional tone (acute, grave, hook above, tilde, dot below).

A hand-typed table of 134 characters has a typo in it that nobody finds until a game ships with
one letter blank.

## Two bugs the tests caught

Worth recording, because both would have shipped as "the font looks a bit odd":

- **`â` and `ã` were drawn identically.** The circumflex and the tilde were both a three-pixel
  caret — pixel for pixel the same mark. `ân` and `ãn` are different words. The tilde is now a
  four-pixel wave, and a test asserts no two composed letters share a bitmap.
- **A blank cell crashed the bounds scan.** `then_some` evaluates its argument eagerly, so a cell
  with no ink underflowed on `max_x - min_x`. The space character is a blank cell, so this was
  reachable on any ordinary sheet.
