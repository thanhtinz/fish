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

## Borrowing the diacritic shapes from a typeface

The letter has to come from the game or the result does not belong. The **mark** does not — a tone
mark is the same shape in every typeface, and drawing one by hand in four pixels gives a blunt
approximation of it. So a font can be pointed at:

```sh
tjlocalizer font projects/game --compose --marks-from ~/fonts/SomeFont.otf
```

The mark is lifted by subtraction: rasterise `ế` and `e` at a size where the base matches the
game's letter, and the mark is what the first has that the second does not. Any font with
Vietnamese letters serves.

**The font is read from where you keep it and never copied into the project.** A font is somebody's
work under somebody's licence, and a localization tool has no business redistributing one.

### Choosing from a folder

Anyone who localizes into Vietnamese has a folder of fonts. Point at it and every font in it is
measured against this game's own sheet:

```sh
tjlocalizer font projects/game --marks-library ~/fonts
```

```
511 fonts readable, 451 cover all 134 Vietnamese letters
Gotham-Ultra supplies 101/134 marks (75%)
```

Ranking is by measurement, not by name, because which font serves best depends on the cell size
and nothing in the file says which. Measured over a real collection at a 12-pixel cell, the spread
runs from 100 of 134 marks down to zero — and the fonts at the top are the heavy weights, whose
marks survive being rasterised that small. The choice is remembered in project.json as a **path**;
no font is copied in.

### Why it is not simply better

A typeface's diacritics are drawn for reading sizes. Rasterised into the cells these games
actually use, they thin out. Measured on a real font, against a sheet with the same letters:

| Cell | Drawn by hand | Borrowed from the typeface |
| --- | --- | --- |
| 12 px | 0 identical pairs | **55 identical pairs** |
| 16 px | 0 | **16** |
| 24 px | 0 | 5 |
| 32 px | 0 | 0 |

Fifty-five identical pairs at 12 px means `à` and `á` are the same picture, and "bà" and "bá" are
the same word on screen. J2ME games use 12 and 16 pixel cells.

So a borrowed mark is kept **only where the letter it produces stays unlike every other one on the
sheet**, and the drawn mark — built for this size — is used everywhere else. The check is per
glyph, against every cell already on the sheet including the game's own, so an invisible mark
cannot make `á` a picture of `a` either. With that rule the same font yields 48 of 134 marks at
12 px and about 101 at 24 px, with no identical pairs at any size.

The report says which: "101 of 134 marks taken from …; the rest were drawn, because a borrowed one
would have made two letters identical."

### The count is not a quality score

A font supplying more marks is not a font producing better ones, and it is worth being blunt about
that because the number invites the opposite reading. Borrowed marks are outlines rasterised
small; drawn ones are shapes designed for this size. Side by side at 12 pixels the drawn marks are
often the more legible — thinner, but shaped, where a heavy typeface's marks arrive as blocks.

So the tool renders both and leaves the judgement to a person, which is what §16 asks for:

```sh
tjlocalizer font projects/game --preview
```

`fonts/preview.png` holds each sample line at the real size and enlarged, with the drawn marks
first and the borrowed ones below. **Look at the small rows.** The enlarged ones are there to see
the shapes; the small ones are what ships.

## From the application

Everything above is in the desktop application's **Font** tab as well as the CLI: finding the
sheet, picking the grid from ranked suggestions, choosing a folder of typefaces and a font from
it, previewing both mark styles at the shipping size, and composing. `crates/tjlocalizer-desktop/
tests/font_commands.rs` covers the orders somebody can click these in - composing before a sheet
is declared, choosing a typeface before a folder - because each of those has to produce a
sentence rather than a panic.

Two measures were added for the interface, since a person picking from a list needs the first
entry to usually be right:

`plausible_grids` scores every grid that divides an image evenly on three counts, because each
one alone accepts a wrong grid. **Boundary clearness** - with the right grid, glyphs do not touch
each other. **Occupancy** - a grid with twice the true number of rows also has clear boundaries,
since it cuts the sheet into the letters and the space above them, but half its cells are empty.
**Baseline agreement** - a grid one and a half times as tall keeps every cell full, and gives
itself away by putting the letters at different depths within them. The third measure exists
because a test caught the second one ranking a 4-row reading of a 6-row sheet first.

`font_candidates` ranks the archive's images by how much each looks like a glyph sheet, on the
best grid's score, how little ink it carries, and how few colours it uses.

## Installing it

Composing writes artwork. Putting it in the game is a rule (§19), because which entry the game
reads and how it maps characters to cells is per-game:

```
tjlocalizer rules <project> --install-font      # written, switched off
tjlocalizer rules <project> --enable install-font
```

Once that rule is enabled *and* its conditions hold, two things change: the built archive carries
the composed sheet, and the coverage the build is judged against becomes that sheet - otherwise
installing a Vietnamese font would still fail the missing-glyph check.

The generated rule covers replacing the image, which is the same in every game. Teaching the game
that the sheet grew taller is not, and is left to a hand-written `setIntConstant` or
`setStringConstant`. See `docs/RULES.md`.

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
- **Borrowed diacritics made 55 pairs of letters identical** at the size these games use. Caught
  by measuring rather than by looking: the marks are individually plausible, and only comparing
  every pair shows that `à` and `á` had become the same bitmap.
- **A blank cell crashed the bounds scan.** `then_some` evaluates its argument eagerly, so a cell
  with no ink underflowed on `max_x - min_x`. The space character is a blank cell, so this was
  reachable on any ordinary sheet.
