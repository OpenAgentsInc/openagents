#!/usr/bin/env bash
# Focused gate for the first gpt-oss chapter; no model downloads or services.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-mxfp4.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" version
python3 -I psionic/tests/mxfp4_oracle.py --header > "$out/mxfp4-oracle.h"
for mode in debug release; do
    flags=(); opt=-O0
    if [[ $mode == release ]]; then flags=(--release); opt=-O2; fi
    "$coax" test psionic/lib/mxfp4.coax "${flags[@]}"
    "$coax" c psionic/lib/mxfp4.coax --library --header "$out/mxfp4.h" "${flags[@]}" > "$out/mxfp4.c"
    for native in plain sanitized; do
        san=()
        if [[ $native == sanitized ]]; then san=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
            -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter \
            "${san[@]}" -I"$out" -Ipsionic/platform "$out/mxfp4.c" \
            psionic/platform/mxfp4-memory.c psionic/tests/mxfp4.c -lm -o "$out/native"
        echo "MXFP4 native $mode $native:"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
    done
    "$coax" run psionic/examples/original-mxfp4.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/original-mxfp4.out "$out/example.out"
done
"$coax" mutate psionic/lib/mxfp4.coax
echo "MXFP4 chapter passes: exact oracle, debug/release, ASan/UBSan, example and zero-survivor mutation"
