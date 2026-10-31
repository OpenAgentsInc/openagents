#!/usr/bin/env bash
# Chapter 97: layer-major CPU chunks, original MoE replay and grouped products.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc}
mutate=0;jobs=${COAX_MUTATE_JOBS:-8}
while [[ $# -gt 0 ]];do case "$1" in --mutate) mutate=1;shift;; --jobs) jobs=$2;shift 2;; *) echo 'usage: check_deepseek_prefill.sh [--mutate] [--jobs N]' >&2;exit 2;;esac;done
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-prefill.XXXXXX");trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/deepseek_prefill_oracle.py
python3 -I psionic/tests/deepseek_prefill_oracle.py --header > "$out/prefill-goldens.h"
python3 -I psionic/tests/deepseek_prefill_record.py
for mode in debug release;do
    flags=(--run-steps 32000000);opt=-O0;if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/deepseek/prefill.coax "${flags[@]}"
    "$coax" c psionic/tests/deepseek-prefill-runtime.coax --library --header "$out/deepseek-prefill.h" "${flags[@]}" > "$out/deepseek-prefill.c"
    cp "$out/deepseek-prefill.h" "$out/deepseek-hot.h";cp "$out/deepseek-prefill.h" "$out/deepseek-expert.h"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -Wno-misleading-indentation "${san[@]}" -I"$out" -Ipsionic/platform "$out/deepseek-prefill.c" psionic/platform/deepseek-prefill-memory.c psionic/platform/deepseek-hot-memory.c psionic/platform/deepseek-expert-memory.c psionic/tests/deepseek_prefill_native.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
    done
    "$coax" run psionic/examples/deepseek-prefill.coax "${flags[@]}" > "$out/example.out";cmp psionic/fixtures/deepseek-prefill.out "$out/example.out"
done
if [[ $mutate == 1 ]];then "$coax" mutate psionic/lib/deepseek/prefill.coax --jobs "$jobs" --run-steps 32000000;fi
if [[ ${PSIONIC_CUDA:-0} == 1 ]];then COAX="$coax" COAX_CC="$cc" psionic/tests/run_deepseek_prefill_cuda.sh;fi
python3 -I psionic/tests/book.py
echo 'Prefill CPU: grouped products, measured-cost chunks, causal layer-major parity, bounded expert fetches, native sanitizers and original MoE replay pass'
