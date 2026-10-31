#!/usr/bin/env bash
# Chapter 125: the pinned nanochat GPT and its inert checkpoint export, CPU binary64.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-nanochat.XXXXXX");trap 'rm -rf "$out"' EXIT
fx=psionic/fixtures/nanochat
python3 -I psionic/tests/nanochat_oracle.py
python3 -I psionic/tests/nanochat_oracle.py --coax | cmp - <(tail -n +2 "$fx/oracle.coax")
python3 -I psionic/tests/nanochat_oracle.py --header > "$out/nanochat-oracle.h"
python3 -I psionic/tests/nanochat_export_test.py
for mode in debug release;do
 flags=();opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
 "$coax" test psionic/lib/nanochat/gpt-checks.coax --steps 2000000000 "${flags[@]+"${flags[@]}"}"
 "$coax" test psionic/lib/nanochat/checkpoint.coax "${flags[@]+"${flags[@]}"}"
 "$coax" c psionic/lib/nanochat/checkpoint.coax --library --header "$out/nanochat.h" "${flags[@]+"${flags[@]}"}" > "$out/nanochat-$mode.c"
 for kind in plain sanitized;do
  san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]+"${san[@]}"}" -I"$out" -Ipsionic/platform "$out/nanochat-$mode.c" psionic/tests/nanochat_native.c -lm -o "$out/native-$mode-$kind"
  ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native-$mode-$kind" "$fx/export/export.json" "$fx/export/model.safetensors" "$fx/export/historical.json"
 done
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/nanochat/checkpoint.coax --root-only --jobs "$jobs";fi
python3 -I psionic/tests/book.py
echo 'nanochat: oracle logits/blocks/loss, cached = full, inert export reproduced and refused, checkpoint admitted, debug/release native plain/ASan/UBSan pass'
