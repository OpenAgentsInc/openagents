#!/usr/bin/env bash
# Chapter 95: owned offline bit/FFN checks and retained original-component replay.
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
        *) echo 'usage: check_deepseek_expert.sh [--mutate] [--jobs N]' >&2;exit 2 ;;
    esac
done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-expert.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_expert_oracle.py --header > "$out/expert-goldens.h"
python3 -I psionic/tests/deepseek_expert_artifacts.py
python3 -I psionic/tests/deepseek_expert_record.py
for mode in debug release;do
    flags=(--run-steps 16000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/deepseek/expert.coax "${flags[@]}"
    "$coax" c psionic/tests/deepseek-expert-runtime.coax --library --header "$out/deepseek-expert.h" "${flags[@]}" > "$out/deepseek-expert.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
            -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -Wno-misleading-indentation "${san[@]}" \
            -I"$out" -Ipsionic/platform "$out/deepseek-expert.c" psionic/platform/deepseek-expert-memory.c \
            psionic/tests/deepseek_expert_native.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native" psionic/fixtures/deepseek/expert-bits.bin
    done
    "$coax" run psionic/examples/deepseek-expert.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/deepseek-expert.out "$out/example.out"
done
if [[ $mutate == 1 ]];then
    "$coax" mutate psionic/lib/deepseek/expert.coax --jobs "$jobs" --run-steps 16000000
fi
python3 -I psionic/tests/book.py
echo 'Expert weights: independent exact bits/grids and FFNs, debug/release, native plain/ASan/UBSan, original expert/MoE replay and book output pass'
