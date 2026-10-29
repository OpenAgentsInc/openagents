#!/usr/bin/env bash
# M0 verification, local/owned machines only. No model downloads or services.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
coax=${COAX:-$root/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-m0.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" version
"$coax" test psionic/lib/learn.coax
"$coax" mutate psionic/lib/learn.coax
for mode in debug release; do
    flags=()
    opt=-O0
    if [[ $mode == release ]]; then flags+=(--release); opt=-O2; fi
    "$coax" c psionic/lib/learn.coax --library --header "$out/learn.h" "${flags[@]}" > "$out/learn.c"
    for native in plain sanitized; do
        sanitizers=()
        if [[ $native == sanitized ]]; then sanitizers=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
            -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter \
            "${sanitizers[@]}" -I"$out" -Ipsionic/platform \
            "$out/learn.c" psionic/platform/memory.c psionic/tests/native.c \
            -lm -o "$out/native"
        echo "native $mode $native:"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
    done
done
python3 -I psionic/tests/book.py --coax "$coax"
echo "Psionic M0 passes: Coax checks, zero-survivor mutation, native debug/release, ASan/UBSan and book"
