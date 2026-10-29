#!/usr/bin/env bash
# Chapter 57: bounded protocol checks; no weights, network or services.
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
        *) echo 'usage: check_deepseek_front.sh [--mutate] [--jobs N]' >&2; exit 2 ;;
    esac
done
if [[ ! -x "$coax" ]]; then coax/build.sh; fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-deepseek-front.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_front_fixture.py
for mode in debug release; do
    flags=(--run-steps 4000000); opt=-O0
    if [[ $mode == release ]]; then flags+=(--release); opt=-O3; fi
    "$coax" test psionic/lib/deepseek/front.coax "${flags[@]}"
    "$coax" c psionic/lib/deepseek/front.coax --library --header "$out/deepseek-front.h" "${flags[@]}" > "$out/deepseek-front.c"
    for kind in plain sanitized; do
        san=()
        if [[ $kind == sanitized ]]; then san=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
            -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" \
            -I"$out" -Ipsionic/platform -Ipsionic/fixtures/deepseek "$out/deepseek-front.c" \
            psionic/platform/deepseek-front-memory.c psionic/tests/deepseek_front_native.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 \
            "$out/native" psionic/fixtures/deepseek/original/r1.pst
    done
    "$coax" run psionic/examples/deepseek-front.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/deepseek-front.out "$out/example.out"
done
if [[ $mutate == 1 ]]; then
    for unit in front-recipe front-stream front-encode; do
        "$coax" mutate "psionic/lib/deepseek/$unit.coax" --jobs "$jobs" --run-steps 4000000
    done
fi
python3 -I psionic/tests/book.py
echo 'DeepSeek chapter 57: protocol oracles, debug/release, all split pairs, native plain/ASan/UBSan, original BPE and book pass'
