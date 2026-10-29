#!/usr/bin/env bash
# Focused complete-model checks; original full inference is explicitly opt-in.
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
        *) echo 'usage: check_deepseek_v2.sh [--mutate] [--jobs N]' >&2;exit 2 ;;
    esac
done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-v2.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_v2_artifacts.py
python3 -I psionic/tests/deepseek_v2_text_artifacts.py
python3 -I psionic/tests/deepseek_v2_record.py
python3 -I psionic/tests/deepseek_v2_fixture.py --header > "$out/v2-tiny-goldens.h"
python3 -I psionic/tests/deepseek_v2_fixture.py --wide --header > "$out/v2-wide-goldens.h"
for mode in debug release;do
    flags=(--run-steps 8000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/tests/deepseek-v2-runtime.coax "${flags[@]}"
    "$coax" c psionic/tests/deepseek-v2-runtime.coax --library --header "$out/deepseek-v2.h" "${flags[@]}" > "$out/deepseek-v2.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
            -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" \
            -I"$out" -Ipsionic/platform "$out/deepseek-v2.c" psionic/platform/deepseek-v2-memory.c \
            psionic/tests/deepseek_v2_native.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native" \
            psionic/fixtures/deepseek/v2-tiny/model.safetensors psionic/fixtures/deepseek/v2-wide/model.safetensors \
            psionic/fixtures/deepseek/v2-original/v2-token-bytes.bin psionic/fixtures/deepseek/v2-cpu-original
    done
    "$coax" run psionic/examples/deepseek-v2.coax "${flags[@]}" > "$out/example.out"
    cmp "$out/example.out" psionic/fixtures/deepseek-v2.out
done
if [[ $mutate == 1 ]];then
    for unit in v2-model v2-weights v2-generate v2-text;do
        "$coax" mutate "psionic/lib/deepseek/$unit.coax" --jobs "$jobs" --run-steps 8000000
    done
fi
echo 'V2 complete model: owned prefix references, admission, token bytes, debug/release and native plain/ASan/UBSan pass'
