#!/usr/bin/env bash
# Chapter 3a only; original models, GPU and full mutation roots stay out.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-tensor-ops.XXXXXX")
trap 'rm -rf "$out"' EXIT
python3 -I psionic/tests/tensor_ops_oracle.py
python3 -I psionic/tests/tensor_ops_oracle.py --header > "$out/tensor-ops-oracle.h"
for mode in debug release;do
    flags=(--run-steps 3000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/tensor-ops-backward.coax "${flags[@]}"
    "$coax" c psionic/lib/tensor-ops-backward.coax --library --header "$out/tensor-ops.h" "${flags[@]}" > "$out/tensor-ops.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
            -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" \
            -I"$out" -Ipsionic/platform "$out/tensor-ops.c" psionic/platform/tensor-ops-memory.c \
            psionic/tests/tensor_ops.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
    done
    "$coax" run psionic/examples/tensor-operators.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/tensor-operators.out "$out/example.out"
done
python3 -I psionic/tests/book.py
echo 'Tensor operators: all 27 families/entries, scalar/finite-difference references, strided layouts, debug/release, native plain/ASan/UBSan and exact book output pass'
