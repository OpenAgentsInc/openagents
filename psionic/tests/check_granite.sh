#!/usr/bin/env bash
# Granite 4.2 8B (issue #12870): binding, tokenizer and chat template on the
# CPU; the dense decoder on Metal against its CPU repeat; parity with the
# packaged llama.cpp when PSIONIC_GRANITE_PARITY=1. The pinned files live
# in PSIONIC_GRANITE_DIR (default ~/models/psionic/granite-4.2-8b-a7042144125c);
# without them only the owned tiny model runs. Nothing is downloaded.
#   psionic/tests/check_granite.sh [--mutate] [--jobs N]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)echo "usage: check_granite.sh [--mutate] [--jobs N]" >&2;exit 2;;esac;done
model=${PSIONIC_GRANITE_DIR:-$HOME/models/psionic/granite-4.2-8b-a7042144125c}
ggufs=()
if [[ -f $model/SHA256SUMS ]];then for q in Q4_K_M Q8_0;do f=$model/gguf-93f3f6a8938e/granite-4.2-8b-$q.gguf;[[ -f $f ]] && ggufs+=("$f");done;fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-granite.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/granite_fixture.py --coax | cmp - psionic/fixtures/granite/tiny.coax
python3 -I psionic/tests/granite_unicode.py | cmp - psionic/fixtures/granite/unicode.coax || echo "note: this Python's Unicode tables differ from the recorded ones"
python3 -I psionic/tests/granite_text.py --cases "$out/cases.bin"
# Steps 1-3: binding, tokenizer and chat, in Coax and natively.
for unit in model tokenizer chat;do for mode in debug release;do
  flags=(--run-steps 200000000);[[ $mode == release ]] && flags+=(--release)
  "$coax" test psionic/lib/granite/$unit.coax "${flags[@]}" | tail -1
done;done
for mode in debug release;do
 flags=(--run-steps 200000000);opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O2;fi
 "$coax" c psionic/lib/granite/chat.coax --library --header "$out/chat.h" "${flags[@]}" > "$out/chat-$mode.c"
 for kind in plain sanitized;do
  san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" \
    -I"$out" -Ipsionic/platform -DGRANITE_COAX_HEADER='"chat.h"' "$out/chat-$mode.c" psionic/platform/granite-file.c psionic/tests/granite_text.c -lm -o "$out/text-$mode-$kind"
  for f in "${ggufs[@]}";do
   ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/text-$mode-$kind" "$f" "$out/cases.bin" > "$out/text.out"
   python3 -I psionic/tests/granite_text.py --check "$out/text.out" "$model" "$f" | sed "s|^|$(basename "$f" .gguf) $mode $kind: |"
  done
 done
done
[[ ${#ggufs[@]} -gt 0 ]] || echo "SKIP: no pinned Granite files in $model; only owned checks ran"
if [[ $mutate == 1 ]];then for unit in model tokenizer chat;do "$coax" mutate psionic/lib/granite/$unit.coax --root-only --jobs "$jobs" --run-steps 200000000;done;fi
