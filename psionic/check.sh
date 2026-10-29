#!/usr/bin/env bash
# Chapters 1–2, local/owned machines only. No model downloads or services.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
coax=${COAX:-$root/coax/bin/coax}
cc=${COAX_CC:-cc}
if [[ ! -x "$coax" ]]; then
    if [[ -n ${COAX:-} ]]; then echo "COAX is not executable: $coax" >&2; exit 1; fi
    coax/build.sh
fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-check.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" version
for unit in learn tensor; do
    "$coax" test "psionic/lib/$unit.coax"
    "$coax" test "psionic/lib/$unit.coax" --release
    "$coax" mutate "psionic/lib/$unit.coax"
done
for mode in debug release; do
    flags=()
    opt=-O0
    if [[ $mode == release ]]; then flags+=(--release); opt=-O2; fi
    for unit in learn tensor; do
        "$coax" c "psionic/lib/$unit.coax" --library --header "$out/$unit.h" "${flags[@]}" > "$out/$unit.c"
        case "$unit" in
            learn) adapter=psionic/platform/memory.c; harness=psionic/tests/native.c ;;
            tensor) adapter=psionic/platform/tensor-memory.c; harness=psionic/tests/tensor.c ;;
        esac
        for native in plain sanitized; do
            sanitizers=()
            if [[ $native == sanitized ]]; then sanitizers=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
            "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
                -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter \
                "${sanitizers[@]}" -I"$out" -Ipsionic/platform \
                "$out/$unit.c" "$adapter" "$harness" -lm -o "$out/$unit-native"
            echo "native $unit $mode $native:"
            ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/$unit-native"
        done
    done
done
python3 -I psionic/tests/book.py --coax "$coax"
echo "Psionic chapters 1–2 pass: Coax debug/release checks, zero-survivor mutation, native debug/release, ASan/UBSan and book"
