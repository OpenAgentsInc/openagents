#!/usr/bin/env bash
# Chapter 8 only. Python/NumPy are independent verification tooling.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
python=${PSIONIC_INTEROP_PYTHON:-python3}
"$python" -c 'import numpy; print("external interchange oracle: NumPy", numpy.__version__)'
if [[ -n ${PSIONIC_INTEROP_OUT:-} ]];then out=$PSIONIC_INTEROP_OUT;mkdir -p "$out";else out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-interchange.XXXXXX");trap 'rm -rf "$out"' EXIT;fi
for mode in debug release;do
    flags=(--run-steps 30000000);opt=-O0
    if [[ $mode == release ]];then flags+=(--release);opt=-O3;fi
    "$coax" test psionic/lib/interchange.coax "${flags[@]}"
    "$coax" c psionic/lib/interchange.coax --library --header "$out/interchange-generated.h" "${flags[@]}" > "$out/interchange-$mode.c"
    common=(-std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -Wno-misleading-indentation -I"$out" -Ipsionic/platform)
    for kind in plain sanitized;do
        san=();if [[ $kind == sanitized ]];then san=(-fsanitize=address,undefined -fno-omit-frame-pointer);fi
        "$cc" "${common[@]}" "${san[@]}" "$out/interchange-$mode.c" psionic/platform/interchange.c psionic/tests/interchange.c -lm -o "$out/native-$mode-$kind"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native-$mode-$kind"
    done
    shared=(-shared);ext=so;if [[ $(uname -s) == Darwin ]];then shared=(-dynamiclib);ext=dylib;fi
    "$cc" "${common[@]}" -fPIC "${shared[@]}" "$out/interchange-$mode.c" psionic/platform/interchange.c -lm -o "$out/libinterchange-$mode.$ext"
    "$python" -I psionic/tests/interchange_numpy.py "$out/libinterchange-$mode.$ext" "$out/continued-$mode.safetensors"
    "$coax" run psionic/examples/model-interchange.coax "${flags[@]}" > "$out/example-$mode.out"
    cmp psionic/fixtures/model-interchange.out "$out/example-$mode.out"
done
python3 -I psionic/tests/book.py
echo 'Interchange: debug/release Coax, native plain/ASan/UBSan, real NumPy DLPack and independent weights round trip pass; no full mutation claim'
