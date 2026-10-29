#!/usr/bin/env bash
# Chapters 45-48: offline owned fixtures; no downloaded code or model required.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-gptoss.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" version
python3 -I psionic/tests/gptoss_fixture.py
python3 -I psionic/tests/gptoss_fixture.py --header > "$out/gptoss-tiny.h"
python3 -I psionic/tests/gptoss_pins.py
for mode in debug release; do
    flags=(--run-steps 2000000); opt=-O0
    if [[ $mode == release ]]; then flags+=(--release); opt=-O3; fi
    for unit in model weights generation speculation tools; do
        "$coax" test "psionic/lib/gptoss/$unit.coax" "${flags[@]}"
    done
    "$coax" c psionic/lib/gptoss.coax --library --header "$out/gptoss.h" "${flags[@]}" > "$out/gptoss.c"
    for native in plain sanitized; do
        san=()
        if [[ $native == sanitized ]]; then san=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
        "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
            -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter \
            "${san[@]}" -I"$out" -Ipsionic/platform "$out/gptoss.c" \
            psionic/platform/gptoss-memory.c psionic/tests/gptoss_native.c -lm -o "$out/native"
        echo "gpt-oss native $mode $native:"
        ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 \
            "$out/native" tiny psionic/fixtures/gptoss/tiny/model.safetensors
    done
    for example in execute-gptoss gptoss-residency speculative-tokens call-a-tool; do
        "$coax" run "psionic/examples/$example.coax" "${flags[@]}" > "$out/$example.out"
        cmp "psionic/fixtures/$example.out" "$out/$example.out"
    done
done
python3 -I psionic/tests/book.py
echo "gpt-oss 45-48 pass: offline references, debug/release, native plain/ASan/UBSan, exact examples and exact book listings"
