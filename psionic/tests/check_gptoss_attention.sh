#!/usr/bin/env bash
# Chapter 52: owned scalar references, bounded CPU code, no model downloads.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-gptoss-attention.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" version
python3 -I psionic/tests/gptoss_oracle.py --header > "$out/gptoss-oracle.h"
for mode in debug release; do
    flags=(); opt=-O0
    if [[ $mode == release ]]; then flags=(--release); opt=-O2; fi
    for unit in gptoss-math gptoss-attention; do
        "$coax" test "psionic/lib/$unit.coax" "${flags[@]}"
    done
    "$coax" c psionic/lib/gptoss-attention.coax --library --header "$out/gptoss-attention.h" "${flags[@]}" > "$out/gptoss-attention.c"
    for native in plain sanitized; do
        san=()
        if [[ $native == sanitized ]]; then san=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
            -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter \
            "${san[@]}" -I"$out" -Ipsionic/platform "$out/gptoss-attention.c" \
            psionic/platform/gptoss-attention-memory.c psionic/tests/gptoss_attention.c -lm -o "$out/native"
        echo "gpt-oss attention native $mode $native:"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/native"
    done
    "$coax" run psionic/examples/attention-sinks.coax "${flags[@]}" > "$out/example.out"
    cmp psionic/fixtures/attention-sinks.out "$out/example.out"
done
for unit in gptoss-math gptoss-attention; do "$coax" mutate "psionic/lib/$unit.coax"; done
echo "gpt-oss chapter passes: independent oracle, debug/release, ASan/UBSan, example and zero-survivor mutation"
