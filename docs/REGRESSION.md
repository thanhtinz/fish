# Seeing what a build changed, and running it

## There is no emulator here

Nothing in this project runs a game. It does not know what a menu looks like, cannot press a
button, and has no opinion about timing. A report from this tool saying "the build looks fine"
would be a claim about work nobody did.

Two things it can honestly do instead.

## Drawing the text, and comparing the drawings

`tjlocalizer proof` draws every approved translation in the game's own glyphs, at the game's own
size, with a marker where the original ended. That already catches what a text report cannot - a
diacritic landing on the letter below it, a stack that smudges at twelve pixels.

What it could not catch until now is *everything else moving*. So a drawing can be accepted:

```
tjlocalizer regress <project> --accept     # this is what it should look like
tjlocalizer regress <project>              # what changed since?
```

The comparison is exact, pixel for pixel, with no tolerance. These are two renderings by the same
code from the same sheet: any difference is a real difference, and a threshold would hide the
one-pixel baseline shift that is the whole reason to look.

```
1184 pixels changed, 0.83% of the picture, in 3 places:
  rows 24-35, 402 pixels
  rows 96-107, 388 pixels
  rows 264-275, 394 pixels
  tests/changed-vi-vn.png
```

Three lines were edited and three bands moved: that is a translator working. Three lines were
edited and *sixty* bands moved: something else changed - a font was recomposed, a glyph order
edited, a rule installed a sheet whose letters sit a pixel lower - and nothing in a diff of
`translations.json` would have shown it.

A drawing that changed size is reported as changed size rather than folded into a pixel count: a
line was added, or the letters got taller.

The picture is written either way, with what changed marked in red over the new drawing rather than
painted solid - a diff image that hides what the text now says hides the thing being checked.

The baseline is accepted by a person, never taken automatically. A baseline captured without
somebody looking records whatever the tool did last time, mistakes included.

## Running it in the emulator you have

Whoever tests these builds has an emulator already. What they lack is not a JVM: it is the tedium
of finding the newest output and typing the command.

```
tjlocalizer play <project> --command emulator --args --device s60 "{game}"
tjlocalizer play <project>
```

The command is recorded in the project and reused. `{game}` is replaced with the build's path;
without it, the path is appended, which is what most emulators expect.

**The command comes from the person, never from the game.** §29's rule is that nothing extracted
from an archive is executed, and a launcher that took its command from a JAR manifest would break
that rule while looking helpful. Nothing here suggests an emulator, downloads one, or reads one out
of anything a stranger wrote.

A project with no emulator recorded says so, and one with no build for the language says *that* -
rather than running whatever build happens to be lying in `output/`.

## In the application

The build tab shows the same comparison, with the marked picture and one button to accept the
current drawing.
