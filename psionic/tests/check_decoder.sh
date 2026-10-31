#!/usr/bin/env bash
# Chapter 113: the tiny causal decoder, CPU binary64.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-decoder.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/decoder_oracle.py
python3 -I psionic/tests/decoder_oracle.py --coax | cmp - <(grep -E '^TD_ORACLE_(LOSS|LOGITS) ::' psionic/lib/transformer/decoder.coax)
python3 -I psionic/tests/decoder_oracle.py --header > "$out/decoder-goldens.h"
for mode in debug release;do
 flags=();opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
 "$coax" test psionic/lib/transformer/decoder.coax "${flags[@]}"
 "$coax" c psionic/lib/transformer/decoder.coax --library --header "$out/decoder.h" "${flags[@]}" > "$out/decoder-$mode.c"
 for kind in plain sanitized;do
  san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" -I"$out" -Ipsionic/platform "$out/decoder-$mode.c" psionic/tests/decoder_native.c -lm -o "$out/native-$mode-$kind"
  ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native-$mode-$kind"
 done
 "$coax" run psionic/examples/causal-decoder.coax "${flags[@]}" > "$out/example-$mode.out"
 cmp psionic/fixtures/causal-decoder.out "$out/example-$mode.out"
done
python3 -I psionic/tests/book.py
echo 'Causal decoder: Python logits, decode = full, causal prefixes, every central difference, training, debug/release native plain/ASan/UBSan and exact example pass'
