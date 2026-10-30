#!/usr/bin/env bash
# Bounded GGUF headers and recorded upstream DATA. CPU only; no model execution.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-gguf.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/gguf_fixtures.py --extract "$out"
python3 -I psionic/tests/gguf_fixtures.py --header > "$out/gguf-goldens.h"
for mode in debug release;do
 flags=(--run-steps 16000000);opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
 "$coax" test psionic/lib/gguf.coax "${flags[@]}"
 "$coax" c psionic/lib/gguf.coax --library --header "$out/gguf.h" "${flags[@]}" > "$out/gguf-$mode.c"
 for kind in plain sanitized;do
  san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" -I"$out" -Ipsionic/platform "$out/gguf-$mode.c" psionic/platform/gguf-memory.c psionic/tests/gguf_native.c -lm -o "$out/native-$mode-$kind"
  ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native-$mode-$kind" "$out"/{owned,layouts,tiny,vocab,big-endian}.gguf
 done
 "$coax" run psionic/examples/gguf.coax "${flags[@]}" > "$out/example-$mode.out"
 cmp psionic/fixtures/gguf.out "$out/example-$mode.out"
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/gguf.coax --jobs "$jobs" --run-steps 16000000;fi
python3 -I psionic/tests/book.py
