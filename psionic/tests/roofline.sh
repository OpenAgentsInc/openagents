#!/usr/bin/env bash
# Chapter 33: measure this host's roofline and record the receipt.
#   psionic/tests/roofline.sh            print the receipt (checks the build under ASan/UBSan first)
#   psionic/tests/roofline.sh --record   also write psionic/fixtures/roofline/<host>.json
# One thread, about ten seconds. Run it on a quiet host; the receipt names
# the machine, compiler and flags, and is never compared byte for byte.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};record=0
while [[ $# -gt 0 ]];do case "$1" in --record)record=1;shift;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-roofline.XXXXXX");trap 'rm -rf "$out"' EXIT
common=(-std=c11 -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas)
"$coax" c psionic/lib/roofline.coax --library --header "$out/roofline.h" > "$out/debug.c"
"$cc" "${common[@]}" -O0 -fsanitize=address,undefined -fno-omit-frame-pointer -I"$out" "$out/debug.c" psionic/platform/roofline.c -lm -o "$out/sanitized"
"$coax" c psionic/lib/roofline.coax --release --library --header "$out/roofline.h" > "$out/release.c"
flags="coax c --release; ${common[*]} -O2"
"$cc" "${common[@]}" -O2 -I"$out" "$out/release.c" psionic/platform/roofline.c -lm -o "$out/roofline"
# The sanitized build runs the same path with smaller timings that are discarded.
ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/sanitized" sanitized sanitized > /dev/null
"$out/roofline" "$("$cc" --version | head -1)" "$flags" > "$out/receipt.json"
python3 -I -c 'import json,sys; json.load(open(sys.argv[1]))' "$out/receipt.json"
cat "$out/receipt.json"
if [[ $record == 1 ]];then mkdir -p psionic/fixtures/roofline;cp "$out/receipt.json" "psionic/fixtures/roofline/$(hostname -s).json";fi
