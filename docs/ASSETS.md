# Words painted into artwork

The quietest failure in this project.

A game's text is not all in its constant pool. Buttons, logos and banners are often artwork with
the words already drawn on, and no amount of translating strings touches them. A build can be
reported as fully translated, pass every check here, and still show a player an English START
button - because that word was never a string.

## Reading the words

A button label in a game is drawn with the game's font, and this project already has that font: a
glyph sheet, every letter in it, pixel for pixel. So the question is not "what letter does this
shape resemble" - which is what general OCR answers, and answers wrongly at twelve pixels - but
"which of these ninety-five exact bitmaps is this shape", which has an answer that can be checked.

```
tjlocalizer assets <project> --read
  btn_start.png  "START"
      every shape matched, worst 0.98
  logo.png  "����" - 4 shapes matched no letter
```

Each shape is cut out of the picture, cropped to its ink, and compared against every glyph of the
sheet by intersection over union, allowing a pixel of drift in each direction because a letter
resaved through a lossy step moves. Above 0.74 it is that letter; below, it is nothing. Letters
that touch are split only where the blob is too wide to be one letter, and spaces are a gap
noticeably wider than the gaps between the letters of the same line - both measured off the image
rather than assumed, because a game draws on its own pitch.

A reading where every shape matched can be accepted in one step:

```
tjlocalizer assets <project> --read --accept
```

A reading with a single unmatched shape in it cannot. `PLA?` is not a word and a person shown it
will accept it anyway, so it is never offered as text - it is shown with the count of what did not
match, and a person types what the picture says.

This needs the project to know which image the game's font is (`tjlocalizer font <project>
--candidates`). Without it there are no letters to match against, and the command says so rather
than falling back to a general reader. Artwork lettered by hand, or in a font the game does not
ship, or scaled or rotated, comes back unread - which is the same answer this tool gave before it
could read anything at all.

## What it does without reading

It lists every image with what its shape suggests, as evidence a person can check by looking:

| Hint | What it means |
| --- | --- |
| `nameSuggests` | Whoever drew the game called the file `start_btn.png`. The weakest evidence and the most often right. |
| `fewColours` | A handful of colours over a small share of the image: lettering, not a scene. |
| `shapeOfALine` | Wide, short, one to three horizontal bands of ink with clear space between them - the shape of a line of writing. |

The hints are facts, not sentences. Two interfaces have to say them and they do not speak the same
language: `tjlocalizer assets` is English and the application is Vietnamese. A core handing out
finished prose would force one of them to show the other's wording.

## What a person does

Looks, and says so:

```
tjlocalizer assets <project> --suspect
tjlocalizer assets <project> --mark start_btn.png --says "START"
tjlocalizer assets <project> --mark start_btn.png --replacement assets/start_btn.png
```

or the same in the application's **Ảnh** tab, where the images are shown rather than named and
**Đọc chữ bằng font game** fills in what it could read.

## Why writing it down is the point

Because the build then holds the project to it. Every marked image is reported at every build
until something replaces it:

```
warn asset.text  start_btn.png has words painted into it (it says "START") and nothing to
                 replace it - the build will ship them untranslated
```

Having a redrawn file is not the same as shipping it. Installing an image is a rule (§19), and a
rule that was written but never switched on leaves the artwork untouched while everything else
says the work is done - so the check compares the bytes in the built archive against the
replacement on disk, and says which of the two situations it found.

Warnings, never errors. Shipping the original artwork is a normal decision: redrawing a logo is
real work and sometimes the answer is "not this release". An error would make the build refuse
over something a person already decided.

## What is not here

Redrawing the image, and laying Vietnamese out inside the original's shape. Both need a person who
can draw. The font engine can render text in the game's own glyphs (`docs/FONTS.md`), which covers
a button whose label is plain lettering, and nothing at all for a logo.
