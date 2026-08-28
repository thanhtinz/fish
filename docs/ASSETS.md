# Assets and provenance

## The supplied asset pack is not used

A ZIP of 94 files was supplied with the brief. 92 of them are raster images; the remaining two
are an Adobe Animate `DOMDocument.xml` and an unrelated `.mp4`.

Reviewing them: they are chibi character / dress-up sprite sheets — hair, robes, boots laid out
as avatar-maker parts — with filenames of the form `images (12).jpeg`, `images (34).jpeg`, plus
a handful of article images scraped from gameplay write-ups. They are not fishing-game art, they
carry no licence or attribution, and their filenames indicate they came from image-search results
rather than from an artist.

**None of it ships.** Two reasons:

1. The brief itself (GDD §2.2) forbids copying artwork, logos, UI composition, or proprietary
   data from the reference title, and states all IP in the project must be newly designed.
2. Redistributing images of unknown provenance in a commercial mobile game is a real legal
   exposure, and app stores act on takedown claims.

If any of that art is in fact owned or licensed by the project, drop it into `assets/` with its
licence and it can replace the generated art in `Art.java` — the drawing code is already isolated
behind one class for exactly that reason.

## What ships instead

| Asset | Source | Licence |
|---|---|---|
| In-game art | Generated at start-up from primitives in `core/.../ui/Art.java` | Project's own |
| Font atlas (`assets/fonts/game.*`) | Baked by `tools/FontGen.java` from DejaVu Sans Bold | Bitstream Vera / DejaVu licence — redistribution permitted |
| Launcher icons, web logo | `tools/IconGen.java`, drawn from primitives | Project's own |
| Species / gear / angler names | Written for this project; real-world fish use their ordinary Vietnamese common names, which are not anyone's IP | Project's own |

Procedural art is a deliberate placeholder, not a final art direction. It keeps the repository
licence-clean, keeps the HTML5 download at ~660 KB, and avoids an atlas pipeline while systems
are still moving. Replacing it means implementing `Art` against a real texture atlas; nothing
above that class changes.

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
