#!/usr/bin/env bash
# Times the pipeline against a game far larger than the ones this tool was written for (§33).
#
# The numbers that matter here are the ones a person waits through: extracting a game's text,
# proposing translations for all of it, building, validating. Run it before and after a change
# that touches any of those.
#
# Usage: tools/bench.sh [resource files] [strings per file]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"

files="${1:-600}"
strings="${2:-200}"

echo "--- $files resource files, $strings strings each, release build"
cargo run -q --release -p tjlocalizer-core --example bench -- "$files" "$strings"
