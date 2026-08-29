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
