#!/usr/bin/env bash
# Chapter 91: owned, offline router equations and margin records.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mutate=0
jobs=${COAX_MUTATE_JOBS:-8}
while [[ $# -gt 0 ]]; do
    case "$1" in
        --mutate) mutate=1;shift ;;
        --jobs) jobs=$2;shift 2 ;;
        *) echo 'usage: check_deepseek_router.sh [--mutate] [--jobs N]' >&2;exit 2 ;;
    esac
done
if [[ ! -x "$coax" ]]; then coax/build.sh;fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-deepseek-router.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_router_oracle.py
for mode in debug release;do
    flags=(--run-steps 4000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/deepseek/router-compare.coax "${flags[@]}"
    "$coax" c psionic/lib/deepseek/router-compare.coax --library --header "$out/deepseek-router.h" "${flags[@]}" > "$out/deepseek-router.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
            -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" \
            -I"$out" -Ipsionic/platform -Ipsionic/fixtures/deepseek "$out/deepseek-router.c" \
            psionic/platform/deepseek-router-memory.c psionic/tests/deepseek_router_native.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
    done
    "$coax" run psionic/examples/deepseek-router.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/deepseek-router.out "$out/example.out"
done
if [[ $mutate == 1 ]];then
    "$coax" mutate psionic/lib/deepseek/router-compare.coax --jobs "$jobs" --run-steps 4000000
fi
python3 -I psionic/tests/book.py
echo 'DeepSeek chapter 91: independent numeric/margin oracle, debug/release, native plain/ASan/UBSan and exact book output pass'
