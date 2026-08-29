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

echo "ok: the missing glyphs were caught, the build refused, and 134 letters were composed"
