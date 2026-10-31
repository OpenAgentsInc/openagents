#!/usr/bin/env bash
# Chapter 93: owned offline hyper-connection equations and temporal handoffs.
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
        *) echo 'usage: check_deepseek_hyper.sh [--mutate] [--jobs N]' >&2;exit 2 ;;
    esac
done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-hyper.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_hyper_oracle.py --header > "$out/hyper-goldens.h"
for mode in debug release;do
    flags=(--run-steps 4000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/deepseek/hyper.coax "${flags[@]}"
    "$coax" c psionic/lib/deepseek/hyper.coax --library --header "$out/deepseek-hyper.h" "${flags[@]}" > "$out/deepseek-hyper.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
            -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" \
            -I"$out" -Ipsionic/platform "$out/deepseek-hyper.c" psionic/platform/deepseek-hyper-memory.c \
            psionic/tests/deepseek_hyper_native.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
    done
    "$coax" run psionic/examples/deepseek-hyper.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/deepseek-hyper.out "$out/example.out"
done
if [[ $mutate == 1 ]];then
    "$coax" mutate psionic/lib/deepseek/hyper.coax --jobs "$jobs" --run-steps 4000000
fi
python3 -I psionic/tests/book.py
echo 'Hyper-connections: independent Decimal/scalar references, all stage handoffs, debug/release, native plain/ASan/UBSan and exact book output pass'
