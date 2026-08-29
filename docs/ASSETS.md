# Words painted into artwork

The quietest failure in this project.

A game's text is not all in its constant pool. Buttons, logos and banners are often artwork with
the words already drawn on, and no amount of translating strings touches them. A build can be
reported as fully translated, pass every check here, and still show a player an English START
button - because that word was never a string.

## What the tool does not do

It does not read the images. There is no OCR, and a wrong reading would be worse than none: a
translator handed "5TART" has to check the picture anyway, and one handed nothing at least knows
they have to.

## What it does

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

or the same in the application's **Ảnh** tab, where the images are shown rather than named.

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

Reading the words, redrawing the image, and laying Vietnamese out inside the original's shape.
The first needs OCR; the other two need a person who can draw. The font engine can render text in
the game's own glyphs (`docs/FONTS.md`), which covers a button whose label is plain lettering, and
nothing at all for a logo.
