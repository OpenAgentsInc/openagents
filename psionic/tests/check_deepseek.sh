#!/usr/bin/env bash
# Chapter 88: focused offline checks. Original weights are opt-in.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mutate=0
jobs=${COAX_MUTATE_JOBS:-8}
while [[ $# -gt 0 ]]; do
    case "$1" in
        --mutate) mutate=1; shift ;;
        --jobs) jobs=$2; shift 2 ;;
        *) echo 'usage: check_deepseek.sh [--mutate] [--jobs N]' >&2; exit 2 ;;
    esac
done
if [[ ! -x "$coax" ]]; then coax/build.sh; fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-deepseek-check.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_fixture.py
python3 -I psionic/tests/deepseek_fixture.py --wide
python3 -I psionic/tests/deepseek_record.py
python3 -I psionic/tests/deepseek_fixture.py --header > "$out/deepseek-tiny.h"
cp psionic/fixtures/deepseek/text-goldens.h "$out/text-goldens.h"
python3 -I psionic/tests/deepseek_artifacts.py
for mode in debug release; do
    flags=(--run-steps 2000000); opt=-O0
    if [[ $mode == release ]]; then flags+=(--release); opt=-O3; fi
    "$coax" test psionic/lib/deepseek.coax "${flags[@]}"
    "$coax" c psionic/tests/deepseek-runtime.coax --library --header "$out/deepseek.h" "${flags[@]}" > "$out/deepseek.c"
    for kind in plain sanitized; do
        san=()
        if [[ $kind == sanitized ]]; then san=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
            -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" \
            -I"$out" -Ipsionic/platform "$out/deepseek.c" psionic/platform/deepseek-memory.c \
            psionic/tests/deepseek_native.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 \
            "$out/native" tiny psionic/fixtures/deepseek/tiny/model.safetensors
        ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 \
            "$out/native" text psionic/fixtures/deepseek/original/r1.pst
    done
    "$coax" run psionic/examples/execute-deepseek.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/execute-deepseek.out "$out/example.out"
done
if [[ $mutate == 1 ]]; then
    # model covers its numerical helpers; text covers plan, binding and recipe.
    for unit in model text; do
        "$coax" mutate "psionic/lib/deepseek/$unit.coax" --jobs "$jobs" --run-steps 2000000
    done
fi
python3 -I psionic/tests/book.py
echo 'DeepSeek chapter 88: debug/release, native plain/ASan/UBSan, original tokenizer, independent owned decoder and book pass'
