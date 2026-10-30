#!/usr/bin/env bash
# Complete small byte language model on owned CC0 data. CPU only.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-byte-lm.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/byte_lm_oracle.py --extract "$out"
python3 -I psionic/tests/byte_lm_oracle.py --header > "$out/byte-lm-goldens.h"
for mode in debug release;do
 flags=(--run-steps 16000000);opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
 "$coax" test psionic/lib/byte-lm.coax "${flags[@]}"
 "$coax" c psionic/lib/byte-lm.coax --library --header "$out/byte-lm.h" "${flags[@]}" > "$out/byte-lm-$mode.c"
 for kind in plain sanitized;do
  san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" -I"$out" -Ipsionic/platform "$out/byte-lm-$mode.c" psionic/platform/byte-lm-memory.c psionic/tests/byte_lm_native.c -lm -o "$out/native-$mode-$kind"
  ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native-$mode-$kind" "$out"/{train.txt,validation.txt,test.txt,expected.bin}
 done
 "$coax" run psionic/examples/byte-lm.coax "${flags[@]}" > "$out/example-$mode.out"
 cmp psionic/fixtures/byte-lm.out "$out/example-$mode.out"
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/byte-lm.coax --jobs "$jobs" --run-steps 16000000;fi
python3 -I psionic/tests/book.py
