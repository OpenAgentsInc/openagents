#!/usr/bin/env bash
# CPU-only bounded tokenizer families and chat templates, recorded upstream DATA.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-tokenizers.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/tokenizer_families_oracle.py --header > "$out/families-goldens.h"
for mode in debug release;do
 flags=(--run-steps 16000000);opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
 "$coax" test psionic/lib/chat-template.coax "${flags[@]}"
 "$coax" c psionic/lib/chat-template.coax --library --header "$out/families.h" "${flags[@]}" > "$out/families-$mode.c"
 for kind in plain sanitized;do
  san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" -I"$out" "$out/families-$mode.c" psionic/tests/tokenizer_families_native.c -lm -o "$out/native-$mode-$kind"
  ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native-$mode-$kind"
 done
 "$coax" run psionic/examples/tokenizer-families.coax "${flags[@]}" > "$out/example-$mode.out"
 cmp psionic/fixtures/tokenizer-families.out "$out/example-$mode.out"
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/chat-template.coax --jobs "$jobs" --run-steps 16000000;fi
python3 -I psionic/tests/book.py
