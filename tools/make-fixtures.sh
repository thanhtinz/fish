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
( cd "$work/jar" && zip -q -r -X "$out/sample-game.jar" . )

echo "wrote $out/SampleGame.class and $out/sample-game.jar"
