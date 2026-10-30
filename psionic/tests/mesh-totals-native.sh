#!/usr/bin/env bash
# Pure reader consumers through the actual emitted C, plain and ASan/UBSan.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
out="${1:-$(mktemp -d "${TMPDIR:-/tmp}/mesh-totals-native.XXXXXX")}"
compiler="${COAX:-$root/coax/bin/coax}"
native_cc="${COAX_CC:-${CC:-cc}}"
mkdir -p "$out"
for name in example maximum; do
  source="$root/psionic/examples/mesh-totals.coax"
  [[ "$name" != maximum ]] || source="$root/psionic/tests/mesh-totals-native.coax"
  "$compiler" c "$source" --release > "$out/$name.c"
  "$native_cc" -std=c11 -O2 -g "$out/$name.c" -lm -o "$out/$name-plain"
  "$out/$name-plain"
  "$native_cc" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer "$out/$name.c" -lm -o "$out/$name-sanitized"
  "$out/$name-sanitized"
done
