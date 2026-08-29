#!/usr/bin/env bash
# Proof that the font engine sees a game that cannot draw its own translation, and fixes it.
#
# This is the failure the rest of the pipeline is blind to: correct text, valid build, blank
# screen. So it is checked end to end rather than only in unit tests.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo build -q -p tjlocalizer-cli
tj="$root/target/debug/tjlocalizer"

# A game whose text comes from a glyph sheet holding printable ASCII and nothing else.
mkdir -p "$work/jar/META-INF"
cargo run -q -p tjlocalizer-core --example font_fixture -- "$work/jar/font.png"
cp "$root/crates/tjlocalizer-core/tests/data/SampleGame.class" "$work/jar/"
cat > "$work/jar/META-INF/MANIFEST.MF" <<'MF'
Manifest-Version: 1.0
MIDlet-Name: Font Game
MIDlet-1: Font Game,,SampleGame
MicroEdition-Configuration: CLDC-1.1
MicroEdition-Profile: MIDP-2.0
MF
( cd "$work/jar" && zip -q -r -X "$work/game.jar" . )

"$tj" import "$work/game.jar" --into "$work/p" --name game --source-language en
"$tj" analyze "$work/p" > /dev/null
"$tj" extract "$work/p" > /dev/null

# A translation the sheet cannot draw a single accented letter of.
node="$(python3 -c "
import json
g = json.load(open('$work/p/content/graph.json'))
print(next(n['id'] for n in g['nodes'] if n['source_text'] == 'Start Game'))
")"
python3 -c "
import json
json.dump({'approved': {'$node': 'Bắt đầu trò chơi'}},
          open('$work/p/translations/vi-vn.json', 'w'), ensure_ascii=False)
"

# Undeclared: a warning, never silence. "Nobody established this" is not "it draws everything".
"$tj" font "$work/p" | grep -q "no font established" \
    || { echo "an undeclared font should say so" >&2; exit 1; }

"$tj" font "$work/p" --sheet font.png --cell 8x12 --columns 16 --lang vi-VN | tee "$work/report"
grep -q "cannot draw" "$work/report" \
    || { echo "the missing glyphs were not reported" >&2; exit 1; }

# And the build must refuse, because it would ship a game showing blanks.
if "$tj" build "$work/p" --lang vi-VN > "$work/build" 2>&1; then
    echo "the build passed despite text the font cannot draw" >&2
    exit 1
fi
grep -q "font.glyph" "$work/build" \
    || { echo "the build failed for some other reason:" >&2; cat "$work/build" >&2; exit 1; }

"$tj" font "$work/p" --compose | tee "$work/compose"
grep -q "composed 134 glyphs" "$work/compose" \
    || { echo "not every letter was composed" >&2; exit 1; }
test -f "$work/p/fonts/extended.png" || { echo "no sheet was written" >&2; exit 1; }

# Composing produces artwork. Until a rule puts it in the archive the game still ships its own
# sheet - so the build must still refuse, or the tool would be calling a picture in a folder a fix.
if "$tj" build "$work/p" --lang vi-VN > "$work/build2" 2>&1; then
    echo "the build passed on a sheet that is not in the game" >&2
    exit 1
fi

"$tj" rules "$work/p" --install-font | tee "$work/rule"
grep -q "install-font \[off\]" "$work/rule" \
    || { echo "a generated rule must start switched off" >&2; cat "$work/rule" >&2; exit 1; }

if "$tj" build "$work/p" --lang vi-VN > "$work/build3" 2>&1; then
    echo "an unenabled rule installed the font anyway" >&2
    exit 1
fi

"$tj" rules "$work/p" --enable install-font > /dev/null
"$tj" build "$work/p" --lang vi-VN | tee "$work/build4" \
    || { echo "the build still failed with the font installed" >&2; cat "$work/build4" >&2; exit 1; }
grep -q "rules: install-font" "$work/build4" \
    || { echo "the build did not record the rule that ran" >&2; exit 1; }

# And with the sheet in the game, its own letters can be measured: this label draws far wider
# than the English it replaces, which no character count would have caught, because the sheet is
# proportional and Vietnamese diacritics cost almost no width at all.
grep -q "layout.width" "$work/build4" \
    || { echo "the label that outgrew its button was not reported" >&2; cat "$work/build4" >&2; exit 1; }

# The proof is in the file that ships, not in the report about it.
python3 - "$work" <<'CHECK'
import sys, zipfile, hashlib, pathlib
work = pathlib.Path(sys.argv[1])
built = next((work / "p/output").glob("*.jar"))
shipped = zipfile.ZipFile(built).read("font.png")
composed = (work / "p/fonts/extended.png").read_bytes()
if shipped != composed:
    raise SystemExit(f"the game shipped a different font.png ({len(shipped)} bytes)")
print(f"shipped font.png is the composed sheet ({len(composed)} bytes, "
      f"sha256 {hashlib.sha256(composed).hexdigest()[:12]})")
CHECK

# And it can be looked at: the text drawn with the game's own glyphs, which is the only way to
# see a mark landing on the letter below it.
"$tj" proof "$work/p" --lang vi-VN --scale 4 > "$work/proof"
sheet="$(head -1 "$work/proof")"
test -s "$sheet" || { echo "no proof sheet was drawn" >&2; cat "$work/proof" >&2; exit 1; }
python3 - "$sheet" <<'SIZE'
import struct, sys
data = open(sys.argv[1], 'rb').read()
w, h = struct.unpack('>II', data[16:24])
if w < 40 or h < 40:
    raise SystemExit(f"the proof sheet is {w}x{h}, which is not a drawing of anything")
print(f"proof sheet {w}x{h}")
SIZE

echo "ok: the missing glyphs were caught, the build refused, 134 letters were composed,"
echo "    and the game ships them only once a rule was written and switched on"
