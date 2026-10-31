#!/usr/bin/env bash
# Chapter 63: pinned DATA, owned references and local native checks only.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-harmony.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" version
python3 -I psionic/tests/harmony_fixtures.py
python3 -I psionic/tests/harmony_oracle.py --header > "$out/harmony-goldens.h"
for mode in debug release; do
    flags=(); opt=-O0
    if [[ $mode == release ]]; then flags=(--release); opt=-O2; fi
    "$coax" test psionic/lib/harmony.coax "${flags[@]}"
    "$coax" c psionic/lib/harmony.coax --library --header "$out/harmony.h" "${flags[@]}" > "$out/harmony.c"
    for native in plain sanitized; do
        san=()
        if [[ $native == sanitized ]]; then san=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
            -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter \
            "${san[@]}" -I"$out" -Ipsionic/platform "$out/harmony.c" \
            psionic/platform/harmony-memory.c psionic/tests/harmony.c -lm -o "$out/native"
        echo "Harmony native $mode $native:"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
    done
    "$coax" run psionic/examples/harmony.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/harmony.out "$out/example.out"
done
"$coax" mutate psionic/lib/harmony/admission.coax
"$coax" mutate psionic/lib/harmony/messages.coax
echo "Harmony chapter passes: pinned ranks, owned oracles, debug/release, ASan/UBSan and zero-survivor mutation"
