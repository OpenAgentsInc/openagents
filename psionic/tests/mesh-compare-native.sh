#!/usr/bin/env bash
# Compile and run the actual private comparison consumer, without Python.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
out="${1:-$(mktemp -d "${TMPDIR:-/tmp}/mesh-compare-native.XXXXXX")}"
compiler="${COAX:-$root/coax/bin/coax}"
native_cc="${COAX_CC:-${CC:-cc}}"
mkdir -p "$out"
"$compiler" c "$root/psionic/examples/mesh-compare.coax" --release > "$out/example.c"
"$native_cc" -std=c11 -O2 -g "$out/example.c" -lm -o "$out/example-plain"
"$out/example-plain" | tee "$out/plain.log"
"$native_cc" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer "$out/example.c" -lm -o "$out/example-sanitized"
"$out/example-sanitized" | tee "$out/sanitized.log"
cmp "$out/plain.log" "$out/sanitized.log"
