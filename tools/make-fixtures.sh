#!/usr/bin/env bash
# Regenerates the committed test fixtures. Needs a JDK; the Rust tests do not.
#
# Targets Java 8 bytecode because that is far closer to the CLDC-era class files this tool is
# built for than a modern default would be, and it keeps the constant pool to the tag subset a
# J2ME game actually uses.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
out="$root/crates/tjlocalizer-core/tests/data"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

javac --release 8 -d "$work" "$here/fixtures/SampleGame.java" 2>/dev/null
mkdir -p "$out"
cp "$work/SampleGame.class" "$out/SampleGame.class"

# A minimal JAR carrying the same class plus a MIDlet-shaped manifest, for archive-level tests.
mkdir -p "$work/jar/META-INF"
cat > "$work/jar/META-INF/MANIFEST.MF" <<'MF'
Manifest-Version: 1.0
MIDlet-Name: Sample Game
MIDlet-Version: 1.0.0
MIDlet-Vendor: Fixture
MIDlet-1: Sample Game,/icon.png,SampleGame
MicroEdition-Configuration: CLDC-1.1
MicroEdition-Profile: MIDP-2.0
MF
cp "$work/SampleGame.class" "$work/jar/"
printf 'level.one.name=Green Field\nlevel.one.hint=Find the key\n' > "$work/jar/levels.properties"

# The fixture is committed and CI checks that regenerating it changes nothing, so it has to be
# byte-reproducible. `zip` stores each entry's local mtime, so without a fixed timestamp and a
# fixed timezone every run produces a different archive; `-r` also walks the directory in readdir
# order, so the entries are listed explicitly instead. 1980-01-01 is the earliest a DOS timestamp
# can express, and is what this project's own writer uses. TZ is pinned for the `touch` as well as
# the `zip`: `touch -t` reads local time, so without it a contributor west of CI would stamp a
# different instant and get a different archive.
rm -f "$out/sample-game.jar"
TZ=UTC find "$work/jar" -exec touch -t 198001010000 {} +
( cd "$work/jar" && TZ=UTC zip -q -X "$out/sample-game.jar" \
    META-INF/MANIFEST.MF SampleGame.class levels.properties )

echo "wrote $out/SampleGame.class and $out/sample-game.jar"
