#!/usr/bin/env bash
# Proof that a package that is not a J2ME JAR goes through the whole pipeline.
#
# The unit tests build their archives in memory. This one writes a real APK to disk, imports it,
# translates it and reads the result back out of the built file - which is the only way to catch a
# format being written back in a shape the game could not load.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo build -q -p tjlocalizer-cli
tj="$root/target/debug/tjlocalizer"

python3 - "$work" <<'BUILD'
import sys, zipfile, pathlib
work = pathlib.Path(sys.argv[1])
with zipfile.ZipFile(work / "game.apk", "w") as z:
    z.writestr("AndroidManifest.xml", b"\x03\x00\x08\x00binary axml")
    z.writestr("classes.dex", b"dex\n035\x00" + bytes(64))
    z.writestr("resources.arsc", bytes(32))
    z.writestr("res/values/strings.xml",
               '<?xml version="1.0" encoding="utf-8"?>\n<resources>\n'
               '    <!-- the main menu -->\n'
               '    <string name="start">Start Game</string>\n'
               '    <string name="shop">Shop</string>\n</resources>\n')
    z.writestr("assets/dialogue.json", '{"lines":[{"text":"You caught a fish!"}]}')
    z.writestr("assets/game.po", '# a comment\nmsgid "Quit"\nmsgstr ""\n')
BUILD

"$tj" import "$work/game.apk" --into "$work/p" --name game --source-language en > /dev/null
test -f "$work/p/original/game.apk" || { echo "the package was not stored under its own kind" >&2; exit 1; }

"$tj" analyze "$work/p" | tee "$work/analyze"
grep -q "Android package" "$work/analyze" || { echo "the package was not recognised" >&2; exit 1; }
grep -q "cannot be rebuilt here" "$work/analyze" \
    || { echo "nothing said it cannot be signed here" >&2; exit 1; }
grep -q "classes.dex" "$work/analyze" \
    || { echo "unreadable text was not named" >&2; exit 1; }

"$tj" extract "$work/p" > /dev/null
python3 - "$work" <<'TRANSLATE'
import sys, json, pathlib
work = pathlib.Path(sys.argv[1])
graph = json.load(open(work / "p/content/graph.json"))
wanted = {"Start Game": "Bắt đầu", "Shop": "Cửa hàng",
          "You caught a fish!": "Bạn câu được một con cá!", "Quit": "Thoát"}
approved = {n["id"]: wanted[n["source_text"]] for n in graph["nodes"]
            if n["source_text"] in wanted}
if len(approved) != len(wanted):
    raise SystemExit(f"only {len(approved)} of {len(wanted)} strings were extracted")
json.dump({"approved": approved}, open(work / "p/translations/vi-vn.json", "w"),
          ensure_ascii=False)
TRANSLATE

"$tj" build "$work/p" --lang vi-VN | tee "$work/build"
grep -q "package.signature" "$work/build" \
    || { echo "the build did not say the result needs signing" >&2; exit 1; }

python3 - "$work" <<'CHECK'
import sys, zipfile, pathlib, json
work = pathlib.Path(sys.argv[1])
built = next((work / "p/output").glob("*"))
if built.suffix != ".apk":
    raise SystemExit(f"the output is {built.name}, not an apk")
z = zipfile.ZipFile(built)

xml = z.read("res/values/strings.xml").decode()
for wanted in ['<string name="start">Bắt đầu</string>', "<!-- the main menu -->",
               '<?xml version="1.0"', '    <string']:
    if wanted not in xml:
        raise SystemExit(f"strings.xml lost {wanted!r}:\n{xml}")

lines = json.loads(z.read("assets/dialogue.json"))
if lines["lines"][0]["text"] != "Bạn câu được một con cá!":
    raise SystemExit(f"the JSON was not patched: {lines}")

po = z.read("assets/game.po").decode()
if 'msgstr "Thoát"' not in po or "# a comment" not in po:
    raise SystemExit(f"the catalogue was not patched in place:\n{po}")

# The parts nothing here can read must come through untouched, not half-written.
for entry in ("classes.dex", "resources.arsc", "AndroidManifest.xml"):
    if z.read(entry) != zipfile.ZipFile(work / "game.apk").read(entry):
        raise SystemExit(f"{entry} was modified")
print("strings.xml, dialogue.json and game.po all patched in place")
CHECK

echo "ok: an Android package was recognised, read, translated and rebuilt, and said what it"
echo "    still needs from a person"
