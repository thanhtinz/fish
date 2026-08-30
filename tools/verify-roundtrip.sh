#!/usr/bin/env bash
# End-to-end proof that a localized class is still a valid, runnable class.
#
# Patches the fixture's string literals, then asks the JVM to load and run the result. A class
# that merely parses proves nothing; one the verifier accepts and executes proves the constant
# pool rewrite is correct.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cargo run -q -p tjlocalizer-core --example patch_demo -- \
    "$root/crates/tjlocalizer-core/tests/data/SampleGame.class" \
    "$work/SampleGame.class"

echo "--- JVM output from the patched class:"
# stdout.encoding, not file.encoding: since Java 19 the console stream follows the former.
java -Dstdout.encoding=UTF-8 -cp "$work" SampleGame

# The other kind of patch: one use of one string changed and the other left alone, which the
# constant pool cannot express. Here the *code* is edited - a load instruction repointed at a new
# constant - so a mistake about instruction lengths would produce a class the verifier rejects
# rather than a wrong string.
mkdir -p "$work/sites"
cargo run -q -p tjlocalizer-core --example site_demo -- \
    "$root/crates/tjlocalizer-core/tests/data/SampleGame.class" \
    "$work/sites/SampleGame.class"

echo "--- JVM output from the class whose code was patched:"
java -Dstdout.encoding=UTF-8 -Xverify:all -cp "$work/sites" SampleGame

# The third kind: a game's font class rewritten to stop drawing from its glyph sheet and let the
# platform draw the letters instead. This one writes a method body, so the verifier's opinion is
# the whole point - a wrong local slot or stack depth is a class the JVM refuses to load.
mkdir -p "$work/font"
cargo run -q -p tjlocalizer-core --example device_font_demo -- \
    "$root/crates/tjlocalizer-core/tests/data/BitmapFont.class" \
    "$work/font/BitmapFont.class"

echo "--- JVM output from the font class switched to the platform's own:"
java -Dstdout.encoding=UTF-8 -Xverify:all -cp "$work/font" BitmapFont
