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
    z.writestr("assets/game.ini", '; menus\n[menu]\ntitle=Options\n')

# An Unreal string table, written the way the engine writes one: a header, a table of namespaces
# and keys, and the strings themselves at the end.
def locres(entries):
    def fstring(text):
        if text.isascii():
            return len(text.encode() + b"\0").to_bytes(4, "little", signed=True) + text.encode() + b"\0"
        units = text.encode("utf-16-le") + b"\0\0"
        return (-(len(units) // 2)).to_bytes(4, "little", signed=True) + units

    body = len(entries).to_bytes(4, "little")          # entry count (version 3)
    body += (1).to_bytes(4, "little")                  # one namespace
    body += (0xAABBCCDD).to_bytes(4, "little") + fstring("Game")
    body += len(entries).to_bytes(4, "little")
    for i, (key, _) in enumerate(entries):
        body += (0x11223344).to_bytes(4, "little") + fstring(key)
        body += (0xDEADBEEF).to_bytes(4, "little")     # source hash
        body += i.to_bytes(4, "little", signed=True)

    magic = bytes([0x0E, 0x14, 0x74, 0x75, 0x67, 0x4A, 0x03, 0xFC,
                   0x4A, 0x15, 0x90, 0x9D, 0xC3, 0x37, 0x7F, 0x1B])
    head = magic + bytes([3])
    out = head + (len(head) + 8 + len(body)).to_bytes(8, "little", signed=True) + body
    out += len(entries).to_bytes(4, "little", signed=True)
    for _, text in entries:
        out += fstring(text) + (1).to_bytes(4, "little", signed=True)
    return out

with zipfile.ZipFile(work / "steam.zip", "w") as z:
    z.writestr("Content/Localization/Game/en/Game.locres",
               locres([("MENU_START", "Start Game"), ("MENU_QUIT", "Quit")]))
    z.writestr("Content/dialogue.json", '{"lines":[{"text":"You caught a fish!"}]}')
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

# And a PC game's zip, which is where Unreal's compiled string table lives.
"$tj" import "$work/steam.zip" --into "$work/steam" --name steam --source-language en > /dev/null
"$tj" analyze "$work/steam" | tee "$work/steam-analyze"
grep -q "unreal-locres" "$work/steam-analyze" \
    || { echo "the Unreal string table was not read" >&2; exit 1; }

"$tj" extract "$work/steam" > /dev/null
python3 - "$work" <<'UNREAL'
import sys, json, pathlib
work = pathlib.Path(sys.argv[1])
graph = json.load(open(work / "steam/content/graph.json"))
wanted = {"Start Game": "Bắt đầu", "You caught a fish!": "Bạn câu được một con cá!"}
approved = {n["id"]: wanted[n["source_text"]] for n in graph["nodes"]
            if n["source_text"] in wanted}
if len(approved) != len(wanted):
    raise SystemExit(f"only {len(approved)} of {len(wanted)} strings were extracted")
json.dump({"approved": approved}, open(work / "steam/translations/vi-vn.json", "w"),
          ensure_ascii=False)
UNREAL

"$tj" build "$work/steam" --lang vi-VN > /dev/null
python3 - "$work" <<'UNREALCHECK'
import sys, zipfile, pathlib, struct
work = pathlib.Path(sys.argv[1])
built = next((work / "steam/output").glob("*"))
data = zipfile.ZipFile(built).read("Content/Localization/Game/en/Game.locres")

# Read the strings back with an independent reader: the point is that the file the game will load
# holds the translation, not that this project agrees with itself.
at = struct.unpack_from("<q", data, 17)[0]
count = struct.unpack_from("<i", data, at)[0]
pos, texts = at + 4, []
for _ in range(count):
    length = struct.unpack_from("<i", data, pos)[0]
    pos += 4
    if length >= 0:
        texts.append(data[pos:pos + length - 1].decode())
        pos += length
    else:
        units = -length
        texts.append(data[pos:pos + units * 2 - 2].decode("utf-16-le"))
        pos += units * 2
    pos += 4  # reference count

if "Bắt đầu" not in texts:
    raise SystemExit(f"the translation is not in the built table: {texts}")
if "Quit" not in texts:
    raise SystemExit(f"an untranslated entry was lost: {texts}")
print(f"the built .locres holds {texts}")
UNREALCHECK

echo "ok: an Android package was recognised, read, translated and rebuilt, and said what it"
echo "    still needs from a person; a PC game's Unreal string table went through as well"
