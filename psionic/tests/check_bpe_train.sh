#!/usr/bin/env bash
# Deterministic bounded Coax tokenizer training; recorded regex oracle, CPU only.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-bpe-train.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/bpe_train_oracle.py --header > "$out/bpe-train-goldens.h"
for mode in debug release;do
 flags=(--run-steps 40000000);opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
 "$coax" test psionic/lib/bpe-train.coax "${flags[@]}"
 "$coax" c psionic/lib/bpe-train.coax --library --header "$out/bpe-train.h" "${flags[@]}" > "$out/bpe-train-$mode.c"
 for kind in plain sanitized;do
  san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" -I"$out" -Ipsionic/platform "$out/bpe-train-$mode.c" psionic/platform/bpe-train-memory.c psionic/tests/bpe_train_native.c -lm -o "$out/native-$mode-$kind"
  ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native-$mode-$kind" psionic/fixtures/text/ch05/qwen35.pst psionic/fixtures/bpe-train/learned.pst "$out/reopened-$mode-$kind.pst"
 done
 "$coax" run psionic/examples/bpe-train.coax "${flags[@]}" > "$out/example-$mode.out"
 cmp psionic/fixtures/bpe-train.out "$out/example-$mode.out"
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/bpe-train.coax --jobs "$jobs" --run-steps 40000000;fi
python3 -I psionic/tests/book.py
