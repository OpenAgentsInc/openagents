#!/usr/bin/env bash
# Focused gate for chapter 231; no models, services or GPU needed.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-soft.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" version
python3 -I psionic/tests/soft_oracle.py
python3 -I psionic/tests/soft_oracle.py --header > "$out/soft-oracle.h"
for mode in debug release; do
    flags=(); opt=-O0
    if [[ $mode == release ]]; then flags=(--release); opt=-O2; fi
    "$coax" test psionic/lib/soft-memory.coax "${flags[@]}"
    "$coax" c psionic/lib/soft-memory.coax --library --header "$out/soft-memory.h" "${flags[@]}" > "$out/soft-memory.c"
    for native in plain sanitized; do
        san=()
        if [[ $native == sanitized ]]; then san=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
            -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter \
            "${san[@]}" -I"$out" -Ipsionic/platform "$out/soft-memory.c" \
            psionic/platform/soft-spans.c psionic/tests/soft.c -lm -o "$out/native"
        echo "Soft native $mode $native:"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
    done
    "$coax" run psionic/examples/softmax-limits.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/softmax-limits.out "$out/example.out"
done
"$coax" mutate psionic/lib/soft-memory.coax --jobs "${PSIONIC_JOBS:-8}"
echo "Softmax chapter passes: independent oracle, debug/release, ASan/UBSan, example and zero-survivor mutation"
