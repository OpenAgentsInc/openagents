#!/usr/bin/env bash
# Chapter 66: original cache grids, BF16 bits and separate codec work budgets.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mutate=0
jobs=${COAX_MUTATE_JOBS:-8}
while [[ $# -gt 0 ]];do
    case "$1" in
        --mutate) mutate=1;shift ;;
        --jobs) jobs=$2;shift 2 ;;
        *) echo 'usage: check_deepseek_cache.sh [--mutate] [--jobs N]' >&2;exit 2 ;;
    esac
done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-cache.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_cache_oracle.py
python3 -I psionic/tests/deepseek_cache_oracle.py --header > "$out/cache-goldens.h"
for mode in debug release;do
    flags=(--run-steps 16000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/deepseek/cache.coax "${flags[@]}"
    "$coax" c psionic/lib/deepseek/cache.coax --library --header "$out/deepseek-cache.h" "${flags[@]}" > "$out/deepseek-cache.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
            -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -Wno-misleading-indentation "${san[@]}" \
            -I"$out" -Ipsionic/platform "$out/deepseek-cache.c" psionic/platform/deepseek-cache-memory.c \
            psionic/tests/deepseek_cache_native.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native" \
            psionic/fixtures/deepseek/cache-bf16.bin psionic/fixtures/deepseek/cache-anchored.bin psionic/fixtures/deepseek/cache-lanes.bin
    done
    "$coax" run psionic/examples/deepseek-cache.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/deepseek-cache.out "$out/example.out"
done
if [[ $mutate == 1 ]];then
    "$coax" mutate psionic/lib/deepseek/cache.coax --jobs "$jobs" --run-steps 16000000
fi
python3 -I psionic/tests/book.py
echo 'Cache: all original grids and BF16 bit patterns, native plain/ASan/UBSan, chunk parity, source ownership budgets and book output pass'
