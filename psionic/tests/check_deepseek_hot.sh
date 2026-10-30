#!/usr/bin/env bash
# Chapter 64: exact CPU cache policy, bounded file misses and expert parity.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc}
mutate=0;jobs=${COAX_MUTATE_JOBS:-8}
while [[ $# -gt 0 ]];do
    case "$1" in
        --mutate) mutate=1;shift ;;
        --jobs) jobs=$2;shift 2 ;;
        *) echo 'usage: check_deepseek_hot.sh [--mutate] [--jobs N]' >&2;exit 2 ;;
    esac
done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-hot.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_hot_oracle.py
python3 -I psionic/tests/deepseek_hot_oracle.py --header > "$out/hot-goldens.h"
python3 -I psionic/tests/deepseek_hot_record.py
for mode in debug release;do
    flags=(--run-steps 16000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/deepseek/hot.coax "${flags[@]}"
    "$coax" c psionic/tests/deepseek-hot-runtime.coax --library --header "$out/deepseek-hot.h" "${flags[@]}" > "$out/deepseek-hot.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        for harness in deepseek_hot_native deepseek_hot_inventory;do
            "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
                -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -Wno-misleading-indentation "${san[@]}" \
                -I"$out" -Ipsionic/platform "$out/deepseek-hot.c" psionic/platform/deepseek-hot-memory.c \
                "psionic/tests/$harness.c" -lm -o "$out/native"
            ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
        done
    done
    "$coax" run psionic/examples/deepseek-hot.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/deepseek-hot.out "$out/example.out"
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/deepseek/hot.coax --jobs "$jobs" --run-steps 16000000;fi
python3 -I psionic/tests/book.py
echo 'Hot experts: exact CPU cache traces and original component replay, bounded mapped/pread misses, native plain/ASan/UBSan and book output pass'
