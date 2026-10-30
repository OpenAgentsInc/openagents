#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
out="${1:-$(mktemp -d "${TMPDIR:-/tmp}/mesh-pool-wire.XXXXXX")}"
compiler="${COAX:-$root/coax/bin/coax}"
native_cc="${COAX_CC:-${CC:-cc}}"
mkdir -p "$out"
"$compiler" c "$root/psionic/tests/mesh-pool-wire-native.coax" --release > "$out/wire.c"
"$native_cc" -std=c11 -O2 -g "$out/wire.c" -lm -o "$out/plain"
"$out/plain" > "$out/plain.jsonl"
"$native_cc" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer "$out/wire.c" -lm -o "$out/sanitized"
"$out/sanitized" > "$out/sanitized.jsonl"
cmp "$out/plain.jsonl" "$out/sanitized.jsonl"
bun "$root/psionic/tests/mesh21/pool_wire_oracle.ts" "$out/sanitized.jsonl"
