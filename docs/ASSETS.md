# Assets and provenance

## Where the art comes from

### The supplied pack (in use)

A ZIP of 94 files was supplied with the brief. The project owner states it was **purchased from
a Chinese vendor and is licensed for use**, and that no live title in Vietnam ships it. Provenance
is theirs to assert - they bought it, this repository did not - and that statement is the basis on
which it is used here. **Keep the purchase record and licence terms**: if the licence is ever
narrower than assumed, everything derived from it lives under `art-src/` and in the fish slots of
the atlas, and can be swapped without touching any code above `Atlas`.

An earlier revision of this document dismissed the whole pack as unusable dress-up art. That was
wrong: it was written after opening a single file and generalising. The pack actually contains
fish sprite sheets, a fishing rod, sea backdrops, character rigs and item icons - most of it
directly relevant to this game.

What it contains, having reviewed all 92 images:

| Content | Roughly | Used |
|---|---|---|
| Monster-fish sprite sheets on chroma backgrounds | 14 sheets | ✅ six archetypes + a legendary-tier fish |
| Fishing rods | 2 | ⬜ not yet - the in-game rod is drawn as a curve so it can bend |
| Sea and cliff backdrops | 14 | ⬜ not yet - candidates for the six zones |
| Character rigs and expression sheets (chroma) | ~10 | ⬜ not yet |
| Item icons, effects, UI bars | ~8 | ⬜ not yet |
| Unrelated (3D room renders, yin-yang symbols, avatar parts) | ~15 | ✖ |

`tools/AssetExtract.java` does the cutting: it keys the chroma background to alpha, flood-fills to
find each connected sprite, and writes it out cropped.

**The original sheets are not committed** - only the cut results, in `art-src/`. The build is
therefore reproducible but the cutting step is not: re-running the tool needs the purchased pack,
which the project owner holds. Keep it somewhere the team can reach, alongside the licence.
Invoke it as `java tools/AssetExtract.java <sheet-dir> <out-dir> [minPixels]`. Two things it learned the hard way, both
now handled - several sheets are letterboxed with black bars, so the four corners agree on black
while the real key colour is the green in between; and JPEG ringing smears the key colour into
sprite edges, so the mask threshold is generous and the edge eroded by a pixel.

### The reference title (not used)

Extracting assets from the reference game's published build was requested and is not done. That is
a live commercial title's copyrighted art, and GDD 2.2 forbids reusing it. This is a separate
matter from the purchased pack above.

## What ships instead

| Asset | Source | Licence |
|---|---|---|
| Fish sprites | Purchased pack, cut by `tools/AssetExtract.java`, packed by `tools/SpriteGen.java` | Per the vendor licence held by the project owner |
| Angler, boat, portraits | Drawn from primitives by `tools/SpriteGen.java` | Project's own |
| Runtime primitives (discs, ripples, gradients) | Generated at start-up in `core/.../ui/Art.java` | Project's own |
| Font atlas (`assets/fonts/game.*`) | Baked by `tools/FontGen.java` from DejaVu Sans Bold | Bitstream Vera / DejaVu licence — redistribution permitted |
| Launcher icons, web logo | `tools/IconGen.java`, drawn from primitives | Project's own |
| Species / gear / angler names | Written for this project; real-world fish use their ordinary Vietnamese common names, which are not anyone's IP | Project's own |

### What the atlas contains

`tools/SpriteGen.java` bakes one 1024x1024 PNG (~105 KB) plus a small JSON of rectangles:

- **Six fish silhouettes**, one per behaviour archetype. Each is constructed separately rather
  than being one body with tweaked numbers — a first pass parameterised a single shape and every
  fish came out looking identical, which defeats the point. The silhouette is the player's fastest
  read on what they have hooked and therefore on how the fight will go: the Runner is a torpedo
  with a deep fork, the Power Tank a blunt slab, the Diver a ray, the Trickster a scalloped sail,
  the Boss spined with a heavy jaw.
- **The player's angler** in two poses, idle and straining, plus a boat. The pose switches on the
  pull level, so the state of the fight is readable from the character before the gauges are.
- **Eight team portraits**, distinguished by hair silhouette, palette and a role accessory rather
  than by facial detail, which does not survive being drawn at 90 px on a phone.

**Fish are drawn at full colour and are not tinted.** An earlier revision baked greyscale
silhouettes and multiplied them by the species' rarity colour; with painted source art that only
mutes it. Rarity now reads as a soft glow behind the fish instead, and the top tiers get their own
sprite so a legendary catch is visibly not the common fish that shares its archetype.

The fish face **left**, towards the angler, so the line anchors on their left edge. Anchoring on
the right ran it to the tail.

The atlas carries **anchors** as well as regions: the angler's grip point is exported so the
renderer knows where to start the rod. The rod is not baked, because a baked rod cannot bend — it
is drawn at runtime as a sampled curve whose bend tracks line tension directly, which is the
clearest feedback the game has.

This is a deliberate art direction rather than a placeholder, but it is not a finished one. A
hand-drawn atlas can replace it by swapping the PNG and JSON; nothing above `Atlas` changes.

## The font, and why it is a bitmap

libGDX's built-in font is ASCII only and the UI is Vietnamese. The usual answer is gdx-freetype
rasterising a TTF at runtime — but FreeType is a native library and **does not exist on the GWT
backend**, which is one of the four required targets. A pre-baked bitmap atlas is the only form
that works identically on desktop, web, Android and iOS.

`tools/FontGen.java` bakes 264 glyphs: ASCII, the full Vietnamese range (every tone and diacritic
combination, plus `đ`/`Đ`), and the symbols the HUD uses.

A character missing from the atlas renders as **nothing** — no error, no fallback box, just a gap
in the text. That failure is silent and would ship, so `FontCoverageTest` checks every authored
string in the content tables and every enum display name against the shipped atlas. If it fails
after adding content, add the characters to the charset in `tools/FontGen.java` and re-run it.

## Trademark note

The working title "Vạn Cân" comes from the supplied GDD. It is close to the title of the
reference game named in that document's own research section. That is a naming decision with
trademark implications, and it is worth a deliberate call before store submission rather than by
default. Nothing in the code depends on the display name: it lives in `gradle.properties`
(`appName`), `android/res/values/strings.xml`, and `ios/Info.plist.xml`. The package identifier
`com.vancan.autofishing` would need changing alongside it, since app IDs cannot change after
publishing.
