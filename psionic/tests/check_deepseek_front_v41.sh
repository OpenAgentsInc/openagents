#!/usr/bin/env bash
# CPU text only. No model weights, CUDA libraries, device queries or services.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc};mutate=0;jobs=${COAX_MUTATE_JOBS:-8}
while [[ $# -gt 0 ]];do case "$1" in --mutate) mutate=1;shift;; --jobs) jobs=$2;shift 2;; *) echo 'usage: check_deepseek_front_v41.sh [--mutate] [--jobs N]' >&2;exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-front-v41.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_front_v41_artifacts.py
python3 -I psionic/tests/deepseek_front_v41_artifacts.py --header > "$out/front-v41-goldens.h"
for mode in debug release;do
    flags=(--run-steps 12000000);opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/deepseek/front-v41-token.coax "${flags[@]}"
    "$coax" c psionic/tests/deepseek-front-v41-runtime.coax --library --header "$out/deepseek-front-v41.h" "${flags[@]}" > "$out/deepseek-front-v41.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" -I"$out" -Ipsionic/platform -Ipsionic/fixtures/deepseek "$out/deepseek-front-v41.c" psionic/platform/deepseek-front-v41-memory.c psionic/tests/deepseek_front_v41_native.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native" psionic/fixtures/deepseek/front-v41.pst
    done
    "$coax" run psionic/examples/deepseek-front.coax "${flags[@]}" > "$out/example.out";cmp psionic/fixtures/deepseek-front.out "$out/example.out"
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/deepseek/front-v41-token.coax --jobs "$jobs" --run-steps 12000000;fi
python3 -I psionic/tests/book.py
echo 'V4.1 front door: pinned original tokenizer, independent text and recipe IDs, actual-token streaming, native sanitizers and exact book output pass; CPU only'
