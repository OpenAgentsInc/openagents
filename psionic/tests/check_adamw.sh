#!/usr/bin/env bash
# AdamW (cpu-adamw-v1): Coax checks, mutation, and native evidence. CPU only.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-adamw.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/adamw_oracle.py
python3 -I psionic/tests/adamw_oracle.py --header > "$out/adamw-oracle.h"
for mode in debug release;do
 flags=();opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
 "$coax" test psionic/lib/optim/adamw.coax "${flags[@]+"${flags[@]}"}"
 "$coax" c psionic/lib/optim/adamw.coax --library --header "$out/adamw.h" "${flags[@]+"${flags[@]}"}" > "$out/adamw-$mode.c"
 for kind in plain sanitized;do
  san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]+"${san[@]}"}" -I"$out" -Ipsionic/platform "$out/adamw-$mode.c" psionic/platform/adamw-memory.c psionic/tests/adamw_native.c -lm -o "$out/native-$mode-$kind"
  ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native-$mode-$kind"
 done
done
"$coax" run psionic/examples/adamw.coax > "$out/example.out";cmp psionic/fixtures/adamw.out "$out/example.out"
"$coax" run psionic/examples/adamw.coax --release > "$out/example-release.out";cmp psionic/fixtures/adamw.out "$out/example-release.out"
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/optim/adamw.coax --jobs "$jobs";fi
python3 -I psionic/tests/book.py
