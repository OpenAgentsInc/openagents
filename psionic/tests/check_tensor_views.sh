#!/usr/bin/env bash
# Chapter 2a only: bounded CPU views and independent native addresses.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-tensor-views.XXXXXX")
trap 'rm -rf "$out"' EXIT
for mode in debug release;do
    flags=();opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/tensor-view.coax "${flags[@]}"
    "$coax" c psionic/lib/tensor-view.coax --library --header "$out/tensor-view.h" "${flags[@]}" > "$out/tensor-view.c"
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
            -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas "${san[@]}" \
            -I"$out" -Ipsionic/platform "$out/tensor-view.c" psionic/platform/tensor-view-memory.c \
            psionic/tests/tensor_view.c -lm -o "$out/native"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
    done
    "$coax" run psionic/examples/tensor-views.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/tensor-views.out "$out/example.out"
done
python3 -I psionic/tests/book.py
echo 'Tensor views: independent integer addresses, ownership/version checks, native plain/ASan/UBSan debug/release and exact example pass'
