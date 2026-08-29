#!/usr/bin/env bash
# End-to-end proof that a game installed as a directory can be translated and patched back.
#
# Everything else in tools/ proves something about a file format somebody else defined, and ends
# with a caveat: no real engine was here to load the result. This one has no such caveat. Every
# byte of the tree below was put there by this script, so "exactly three of twenty thousand files
# were read", "exactly three were written back" and "nothing else was touched" are provable
# completely rather than argued for - and those three are the whole claim.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo build -q -p tjlocalizer-cli
tj="$root/target/debug/tjlocalizer"

# A game shaped the way one is: almost entirely things nobody translates.
python3 - "$work" <<'BUILD'
import sys, pathlib
game = pathlib.Path(sys.argv[1]) / "game"

for i in range(10_000):
    d = game / f"Content/Textures/set{i // 500:02d}"
    d.mkdir(parents=True, exist_ok=True)
    (d / f"tex{i:05d}.png").write_bytes(b"\x89PNG\r\n\x1a\n" + bytes(48))
for i in range(10_000):
    d = game / f"Content/Audio/bank{i // 500:02d}"
    d.mkdir(parents=True, exist_ok=True)
    (d / f"vo{i:05d}.ogg").write_bytes(b"OggS" + bytes(48))

(game / "Engine/Binaries/Win64").mkdir(parents=True, exist_ok=True)
(game / "Engine/Binaries/Win64/Fishing.exe").write_bytes(b"MZ" + bytes(4096))
(game / "steam_api64.dll").write_bytes(bytes(512))

# The three files that matter.
(game / "Content/Localization/Game/en").mkdir(parents=True, exist_ok=True)
(game / "Content/Localization/Game/en/Game.po").write_text(
    '# somebody wrote this comment\nmsgid "Start Game"\nmsgstr ""\n\nmsgid "Quit"\nmsgstr ""\n',
    encoding="utf-8")
(game / "Content/dialogue.json").write_text(
    '{"lines":[{"text":"You caught a fish!"}]}', encoding="utf-8")
(game / "Content/settings.ini").write_text(
    '; the options menu\n[menu]\ntitle=Options\n', encoding="utf-8")

# And one text file too big to read, which has to be reported rather than dropped quietly.
(game / "Content/telemetry.json").write_text("[" + ",".join('{"e":%d}' % i for i in range(900_000)) + "]",
                                             encoding="utf-8")
BUILD

"$tj" import "$work/game" --into "$work/p" --name fishing --source-language en | tee "$work/import"

# The proportion is the feature. Both numbers, in this order.
grep -q "20006 files" "$work/import" || { echo "the scan did not see the whole tree" >&2; exit 1; }
grep -q "read 3 " "$work/import" || { echo "the ingest did not read exactly the three text files" >&2; exit 1; }
# A texture is not reported; a text file skipped for its size is.
grep -q "Content/telemetry.json" "$work/import" || { echo "the oversized JSON was dropped silently" >&2; exit 1; }
if grep -q "tex00000.png" "$work/import"; then
    echo "textures were listed as skipped, which buries what matters" >&2
    exit 1
fi

"$tj" analyze "$work/p" | tee "$work/analyze"
grep -q "^directory" "$work/analyze" || { echo "the game was not recognised as a directory" >&2; exit 1; }
grep -q "Steam API" "$work/analyze" || { echo "the engine evidence did not survive to analyze" >&2; exit 1; }

"$tj" extract "$work/p" > /dev/null
python3 - "$work" <<'TRANSLATE'
import sys, json, pathlib
work = pathlib.Path(sys.argv[1])
graph = json.load(open(work / "p/content/graph.json"))
wanted = {"Start Game": "Bắt đầu", "Quit": "Thoát",
          "You caught a fish!": "Bạn câu được một con cá!", "Options": "Tuỳ chọn"}
approved = {n["id"]: wanted[n["source_text"]] for n in graph["nodes"] if n["source_text"] in wanted}
if len(approved) != len(wanted):
    raise SystemExit(f"only {len(approved)} of {len(wanted)} strings were extracted")
json.dump({"approved": approved}, open(work / "p/translations/vi-vn.json", "w"), ensure_ascii=False)
TRANSLATE

"$tj" build "$work/p" --lang vi-VN > /dev/null
patch="$work/p/output/fishing-vi-vn"
test -d "$patch" || { echo "a directory game did not build to a directory" >&2; exit 1; }
test -f "$patch/patch.json" || { echo "the patch has no manifest" >&2; exit 1; }
test -f "$patch/INSTALL.txt" || { echo "the patch has no instructions" >&2; exit 1; }

# Only what changed. This is the assertion the whole design exists for.
test ! -e "$patch/Content/Textures" || { echo "the patch carries textures" >&2; exit 1; }
test ! -e "$patch/Engine" || { echo "the patch carries the game binary" >&2; exit 1; }
files="$(find "$patch" -type f ! -name 'patch.json' ! -name 'INSTALL.txt' | wc -l)"
test "$files" -eq 3 || { echo "the patch holds $files files, not the 3 that changed" >&2; exit 1; }

# Applied to a copy, never to the tree the project was imported from.
cp -r "$work/game" "$work/copy"
"$tj" apply-patch "$work/p" --to "$work/copy" --dry-run | tee "$work/dry"
grep -q "nothing was written" "$work/dry" || { echo "the dry run did not say it wrote nothing" >&2; exit 1; }
cmp -s "$work/game/Content/settings.ini" "$work/copy/Content/settings.ini" \
    || { echo "the dry run wrote something" >&2; exit 1; }

"$tj" apply-patch "$work/p" --to "$work/copy"

# The game reads Vietnamese, keeps its comments, and everything else is byte-identical.
grep -q "title=Tuỳ chọn" "$work/copy/Content/settings.ini" || { echo "the INI was not patched" >&2; exit 1; }
grep -q "; the options menu" "$work/copy/Content/settings.ini" || { echo "the INI lost its comment" >&2; exit 1; }
grep -q 'msgstr "Bắt đầu"' "$work/copy/Content/Localization/Game/en/Game.po" || { echo "the catalogue was not patched" >&2; exit 1; }

python3 - "$work" <<'COMPARE'
import sys, pathlib, hashlib
work = pathlib.Path(sys.argv[1])
before, after = work / "game", work / "copy"
changed = sorted(
    str(f.relative_to(before))
    for f in before.rglob("*") if f.is_file()
    if hashlib.sha256(f.read_bytes()).digest()
    != hashlib.sha256((after / f.relative_to(before)).read_bytes()).digest()
)
expected = ["Content/Localization/Game/en/Game.po", "Content/dialogue.json", "Content/settings.ini"]
if changed != expected:
    raise SystemExit(f"the wrong files changed: {changed}")
print(f"{len(list(before.rglob('*')))} entries in the game, {len(changed)} files changed")
COMPARE

# What it replaced was kept, and puts the game back exactly.
backup="$work/p/builds/vi-vn/0001/backup"
test -f "$backup/Content/settings.ini" || { echo "nothing was backed up" >&2; exit 1; }
cp -r "$backup/." "$work/copy/"
cmp -s "$work/game/Content/settings.ini" "$work/copy/Content/settings.ini" \
    || { echo "the backup did not restore the original" >&2; exit 1; }

# A patch built from one copy of a game must not be written over another, and must not write
# *part* of itself first - a half-patched game is in a state neither the patch nor the backup
# describes.
cp -r "$work/game" "$work/other"
echo "; edited by somebody" >> "$work/other/Content/settings.ini"
before_json="$(sha256sum "$work/other/Content/dialogue.json" | cut -d' ' -f1)"
if "$tj" apply-patch "$work/p" --to "$work/other" > "$work/refused" 2>&1; then
    echo "a patch was applied to a game it was not built from" >&2
    exit 1
fi
grep -q "settings.ini" "$work/refused" || { echo "the refusal did not name the file" >&2; exit 1; }
grep -q "none of it was applied" "$work/refused" || { echo "the refusal did not say it wrote nothing" >&2; exit 1; }
test "$(sha256sum "$work/other/Content/dialogue.json" | cut -d' ' -f1)" = "$before_json" \
    || { echo "a file was written despite the patch being refused" >&2; exit 1; }

# And a game updated behind the tool's back is named, because that is the quietest failure here.
echo '{"lines":[{"text":"You caught a bigger fish!"}]}' > "$work/game/Content/dialogue.json"
"$tj" build "$work/p" --lang vi-VN | tee "$work/drift"
grep -q "tree.drift" "$work/drift" || { echo "a game updated since import was not reported" >&2; exit 1; }

echo "ok: a 20 006-file game folder was read three files deep, translated, and written back as a"
echo "    three-file patch; everything else came out byte-identical, what was replaced restores"
echo "    the game exactly, a patch for a different copy was refused whole, and a game updated"
echo "    behind the tool's back was named"
