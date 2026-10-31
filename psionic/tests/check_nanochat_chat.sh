#!/usr/bin/env bash
# nanochat conversation tokens over the chapter 77 byte BPE: independent oracle, Coax debug/release, native plain and ASan/UBSan.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};mutate=0;jobs=4
while [[ $# -gt 0 ]];do case "$1" in --mutate)mutate=1;shift;;--jobs)jobs=$2;shift 2;;*)exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-nanochat-chat.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/nanochat_chat_oracle.py
for mode in debug release;do
 flags=();opt=-O0;if [[ $mode == release ]];then flags=(--release);opt=-O3;fi
 "$coax" test psionic/lib/nanochat-chat.coax "${flags[@]+"${flags[@]}"}" | tail -1
 "$coax" c psionic/lib/nanochat-chat.coax --library --header "$out/nanochat-chat.h" "${flags[@]+"${flags[@]}"}" > "$out/nanochat-chat-$mode.c"
 for kind in plain sanitized;do
  san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]+"${san[@]}"}" -I"$out" -Ipsionic/platform "$out/nanochat-chat-$mode.c" psionic/tests/nanochat_chat.c -lm -o "$out/native-$mode-$kind"
  ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native-$mode-$kind" psionic/fixtures/bpe-train/learned.pst
 done
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/nanochat-chat.coax --root-only --jobs "$jobs" | tail -1;fi
