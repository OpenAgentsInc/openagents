#!/usr/bin/env bash
# Chapter 90 component gate. Complete V2 execution remains a separate gate.
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
        *) echo 'usage: check_deepseek_v2_math.sh [--mutate] [--jobs N]' >&2;exit 2 ;;
    esac
done
if [[ ! -x "$coax" ]];then coax/build.sh;fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-v2-math.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_mla_oracle.py
python3 -I psionic/tests/deepseek_v2_rope_oracle.py
for mode in debug release;do
    flags=(--run-steps 4000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/tests/deepseek-v2-math-runtime.coax "${flags[@]}"
    "$coax" c psionic/tests/deepseek-v2-math-runtime.coax --library --header "$out/deepseek-v2-math.h" "${flags[@]}" > "$out/deepseek-v2-math.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
            -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" \
            -I"$out" -Ipsionic/platform -Ipsionic/fixtures/deepseek "$out/deepseek-v2-math.c" \
            psionic/platform/deepseek-v2-math-memory.c psionic/tests/deepseek_v2_math_native.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
    done
done
if [[ $mutate == 1 ]];then
    for unit in mla v2-rope;do "$coax" mutate "psionic/lib/deepseek/$unit.coax" --jobs "$jobs" --run-steps 4000000;done
fi
echo 'V2 component gate: compressed/expanded MLA, original-parameter YaRN, debug/release and native plain/ASan/UBSan pass'
