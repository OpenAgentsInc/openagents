#!/usr/bin/env bash
# Chapter 121: feed a single-threaded consumer from a loader with 1, 2, 4
# and 8 worker threads and rings of 1, 2 and 4 batches, and record the receipt.
#   psionic/tests/feed.sh            print the receipt (checks a small run under ASan/UBSan and TSan first)
#   psionic/tests/feed.sh --record   also write psionic/fixtures/feed/<host>.json
# The harness exits nonzero if any run's bytes differ from the
# single-threaded pk_next stream, or if a resume from the consumer's cursor
# does. The receipt names the machine, compiler, flags and load, and is
# never compared byte for byte.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};record=0
while [[ $# -gt 0 ]];do case "$1" in --record)record=1;shift;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-feed.XXXXXX");trap 'rm -rf "$out"' EXIT
common=(-std=c11 -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unused-variable -Wno-unknown-pragmas)
"$coax" c psionic/lib/data/feed.coax --library --header "$out/feed.h" > "$out/debug.c"
"$cc" "${common[@]}" -O0 -fsanitize=address,undefined -fno-omit-frame-pointer -I"$out" "$out/debug.c" psionic/platform/feed.c -lm -pthread -o "$out/sanitized"
"$cc" "${common[@]}" -O1 -fsanitize=thread -I"$out" "$out/debug.c" psionic/platform/feed.c -lm -pthread -o "$out/thread"
"$coax" c psionic/lib/data/feed.coax --release --library --header "$out/feed.h" > "$out/release.c"
flags="coax c --release; ${common[*]} -O2 -pthread"
"$cc" "${common[@]}" -O2 -I"$out" "$out/release.c" psionic/platform/feed.c -lm -pthread -o "$out/feed"
# Small runs check memory, undefined behaviour and data races; their receipts are discarded.
ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/sanitized" sanitized sanitized small > /dev/null
TSAN_OPTIONS=halt_on_error=1 "$out/thread" thread thread small > /dev/null
"$out/feed" "$("$cc" --version | head -1)" "$flags" > "$out/receipt.json"
python3 -I -c 'import json,sys; json.load(open(sys.argv[1]))' "$out/receipt.json"
cat "$out/receipt.json"
if [[ $record == 1 ]];then mkdir -p psionic/fixtures/feed;cp "$out/receipt.json" "psionic/fixtures/feed/$(hostname -s).json";fi
