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


## Finding an emulator, without fetching one

`play` runs a command the project's owner recorded. That is the right shape — nothing read out of a
game can influence what gets executed — but it puts the whole of "which emulator, where is it, what
arguments" on somebody who only wanted to see their translation on a screen.

So `--find` looks, and **only** looks:

```
tjlocalizer play <project> --find
emulators on this machine:
  FreeJ2ME           /home/you/Downloads/freej2me.jar
      freej2me.jar in /home/you/Downloads

to use the first of these:
    tjlocalizer play <project> --use-found
```

It searches `PATH` for programs and the handful of directories people actually keep an emulator jar
in for FreeJ2ME, MicroEmulator, KEmulator and the Java ME SDK. A jar gets `java -jar <it> {game}`
built for it, and is only offered where there is a `java` to run it with — an emulator that cannot
start is not a find.

**Nothing is downloaded, installed or suggested to download.** That rule is why the empty answer
matters more than the full one, and why it names every place it looked:

```
no J2ME emulator found on this machine. Looked in:
  /usr/local/bin
  /usr/bin
  /home/you/Downloads
  ...

nothing is downloaded here. Install one yourself, then point this at it:
    tjlocalizer play <project> --command <program> --args ...
```

"No emulator found" is not something anybody can act on. A list of the places that were checked is.

## The journal

Localizing a game is not one sitting. Somebody imports a game on Sunday, extracts the text, approves
forty lines, and comes back three weeks later to a folder of JSON that says perfectly what the state
*is* and nothing at all about how it got there or why they stopped.

So the milestones record themselves into `journal.jsonl` in the project root — one JSON object per
line, appended, never rewritten:

```
tjlocalizer status <project>
sample-game  (projects/sample-game)
  source   original/sample-game.jar
  vi-VN    128/412 approved, build 3 reported 2 error(s)

what happened, most recent last:
  2026-08-31T15:42:02Z  import   imported sample-game (241033 bytes)
  2026-08-31T15:42:02Z  extract  412 text nodes, 388 of them for a translator
  2026-08-31T15:44:19Z  note     waiting on a screenshot of the shop menu
  2026-08-31T15:51:07Z  build [vi-VN]  build 3: 128 translations applied, validation reported 2 error(s)
```

What is recorded is what is worth reading in a month: imports, extractions, builds **and whether
they passed**, rules switched on or off, patches applied and what they overwrote, languages added or
set aside. Not "the project is 60% done" — a number that was true for one afternoon.

`tjlocalizer note <project> "..."` adds the one thing no recorded milestone can know: why somebody
stopped. That is not derivable from any file in the project.

Three properties are load-bearing, and each has a test:

- **Append-only.** A log that is read, edited and rewritten is a log that can lose an entry, and the
  entry it loses is the one from the day something went wrong.
- **A corrupt line does not take the good ones with it.** A truncated last line — a power cut
  mid-write — must not hide every entry before it.
- **A journal that cannot be written never fails the work.** A build that worked is not a broken
  build because a note about it could not be appended.
