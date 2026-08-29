#!/usr/bin/env bash
# End-to-end proof that the CLI produces a JAR a JVM will run.
#
# The unit tests check each stage; this checks the thing that actually ships. It drives the real
# subcommands over the fixture and then asks the JVM to load and execute the localized class,
# because a JAR that repacks cleanly and still fails verification is the failure that matters.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo build -q -p tjlocalizer-cli
tj="$root/target/debug/tjlocalizer"
jar="$root/crates/tjlocalizer-core/tests/data/sample-game.jar"

# Two targets, so the multi-language path is exercised rather than assumed.
"$tj" import "$jar" --into "$work/demo" --name sample-game --target vi-VN,th --source-language en
"$tj" analyze "$work/demo"
"$tj" extract "$work/demo"
"$tj" dictionaries "$work/demo"

# Seed the memory the way a previous project would have, so there is something to approve. The
# file is named for the direction: a project can hold several, and they must not collide.
cat > "$work/demo/memory/en-vi-vn.json" <<'JSON'
{ "entries": [
  { "source": "Start Game", "target": "Bắt đầu trò chơi", "context": null },
  { "source": "Quit", "target": "Thoát", "context": null },
  { "source": "HP: %d / %d", "target": "Sinh lực: %d / %d", "context": null }
] }
JSON

"$tj" translate "$work/demo" --lang vi-VN --apply-safe --gloss
"$tj" build "$work/demo" --all
"$tj" builds "$work/demo"
"$tj" test "$work/demo/output/sample-game-vi-vn.jar"

# Exporting to a path of the user's choosing is the whole point of a desktop tool; the CLI runs
# the same copy so it is checked here too.
mkdir -p "$work/out"
"$tj" export "$work/demo" "$work/out" --all
test -f "$work/out/sample-game-vi-vn.jar" || { echo "export did not write the Vietnamese build" >&2; exit 1; }
test -f "$work/out/sample-game-th.jar" || { echo "export did not write the Thai build" >&2; exit 1; }

echo "--- JVM output from the localized JAR:"
# stdout.encoding, not file.encoding: since Java 19 the console stream follows the former.
out="$(java -Dstdout.encoding=UTF-8 -cp "$work/demo/output/sample-game-vi-vn.jar" SampleGame)"
echo "$out"

# Translated text present, untranslated text intact, and the format string and resource path
# untouched - patching those is the failure mode this whole design exists to avoid.
for expected in "Bắt đầu trò chơi" "Thoát" "Sinh lực: %d / %d" "You have arrived at last, traveller." "/img/hud.png"; do
    grep -qF "$expected" <<<"$out" || { echo "MISSING from JVM output: $expected" >&2; exit 1; }
done
if grep -qF "Start Game" <<<"$out"; then
    echo "English survived where it should not have" >&2
    exit 1
fi

echo "ok: the localized JAR loads, verifies and runs"
