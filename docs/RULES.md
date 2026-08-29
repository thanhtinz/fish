# The rule engine

Everything else in this project is general. It works on any JAR because it only does things that
are true of every JAR: text lives in the constant pool, resources are entries, a manifest is
`Key: value`. That generality is the point of §2, and it is why nothing in the core knows the name
of a game.

Some of the work is not like that.

Making a game *use* a new glyph sheet means knowing that this game reads `/font.png`, that this
class holds the number of columns, that this string lists the characters in sheet order. Nothing
can infer those, and a tool that guessed would corrupt games - quietly, one constant at a time, in
a way that surfaces as a rendering bug in a build nobody can explain.

So it is written down instead, as data.

## What a rule is

```json
{
  "id": "install-font",
  "description": "Replace font.png with the sheet holding the Vietnamese letters.",
  "enabled": false,
  "when": [
    { "kind": "entrySha256", "entry": "font.png", "sha256": "bbe173cc…" },
    { "kind": "projectFile", "path": "fonts/extended.png" }
  ],
  "then": [
    { "kind": "replaceEntry", "entry": "font.png", "from": "fonts/extended.png" }
  ]
}
```

`when` is what the rule expects to find. The engine checks it against the actual archive and
refuses when it does not hold, so a rule carried over from another version of a game reports that
the game is not what it was written for rather than patching the wrong thing. `entrySha256` is the
strictest and the right one for a file swap: it says *this is the image I measured*.

`then` is what it would change. Four actions, deliberately:

| Action | What it does |
| --- | --- |
| `replaceEntry` | Puts a file from the project directory into the archive. |
| `setIntConstant` | Changes every `CONSTANT_Integer` of one value, in one named class. |
| `setStringConstant` | Changes a string literal, in one named class. |
| `setStringAtSite` | Changes what one named *method* loads, leaving the string itself alone. |

Every constant action is scoped to a class **and** to an exact previous value. "Change the 16 to
22" applied across a whole game changes sixteens that had nothing to do with the font.

### `setStringAtSite`, and why it exists

A game shows `Back` on eleven screens from one constant. A translation that has to differ on one of
them - because Vietnamese wants `Quay lại` in a menu and `Trở về` after a battle - has nowhere in
the pool to say so: rewriting the constant changes all eleven.

```json
{
  "kind": "setStringAtSite",
  "class": "GameScreen.class", "method": "drawBattleEnd",
  "from": "Back", "to": "Trở về"
}
```

This adds a new constant and points the load instructions *in that one method* at it. The plan says
both halves before it runs:

```
would in GameScreen.class.drawBattleEnd, load "Trở về" instead of "Back" at 1 place
       (10 other uses of "Back" left alone)
```

## What a rule cannot do

It cannot add bytecode. Every action here is something this crate already does and has verified on
a real JVM - rewriting the constant pool, replacing an entry, repointing one load instruction - so
no rule can make a class fail verification. `setStringAtSite` changes an operand and never a
length, so every jump, exception range and stack map frame in the method stays exactly as the
compiler left it; where a new constant would not fit the instruction's one-byte operand it is
refused rather than widened, because widening moves everything after it. A patch that needs new
instructions is not expressible, on purpose.

## Nothing runs because it was written

`enabled` starts false, including on rules this tool generates itself. A rule changes how somebody
else's game behaves; that is a decision, and it is theirs.

`tjlocalizer rules <project>` shows every rule with what it would do, in numbers read from *this*
archive rather than repeated from the rule:

```
install-font [off]
  Replace font.png with the sheet holding the Vietnamese letters. …
  would replace font.png (4227 bytes) with fonts/extended.png (11812 bytes)
```

A rule matching nothing produces no effect lines and is reported as not ready. The difference
between a patch and a wish is whether the game actually contains what it claims to change.

## The font, end to end

This is what the rule engine was built for. The font engine could compose the 134 Vietnamese
letters from a game's own glyphs, and then stop - the artwork sat in `fonts/` and the game shipped
its original sheet, so a build with perfect Vietnamese still failed its own glyph check.

```
tjlocalizer font  <project> --sheet font.png --cell 8x12 --columns 16
tjlocalizer font  <project> --compose
tjlocalizer rules <project> --install-font      # writes it, switched off
tjlocalizer rules <project> --enable install-font
tjlocalizer build <project> --lang vi-VN
```

Two things follow from the rule being enabled, and both matter:

- The archive that ships carries the composed sheet. `tools/verify-font.sh` opens the built JAR and
  compares the bytes, because a report saying it happened is not the same as it happening.
- The coverage the build is *judged* against becomes the sheet that ships. Without that, installing
  a Vietnamese font would still fail the missing-glyph check, and the tool would be refusing a
  build that is in fact correct.

Only an enabled rule whose conditions hold counts for either. One written for a different version
of the artwork changes neither.

## The half this does not write

`--install-font` generates the part that is the same in every game: replace the image. The other
part - teaching the game that the sheet now has more rows, and which character each new cell holds
- is per-game code, and is left for a person to add as `setIntConstant`, `setStringConstant` or
`setStringAtSite`.

The generated rule's description says so, and the interface repeats it where the button is. A rule
that swapped the artwork and stopped would leave a game drawing its old letters out of a taller
sheet: a display bug rather than a missing feature, and much harder to notice.

## Where they live

`rules/rules.json` in the project directory, which is a folder people commit and share. A rule
contains no keys and no absolute paths - `from` is relative to the project - so it travels.
