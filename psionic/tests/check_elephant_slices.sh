#!/usr/bin/env bash
# Original-weight slices of Elephant, checkable in seconds: the embedding
# gather, linear layer 0, full-attention layer 3 and readout rows 0..4095 over
# five tokens, against references recorded once from the Psionic CPU path.
#   psionic/tests/check_elephant_slices.sh ORIGINAL_DIRECTORY
#   psionic/tests/check_elephant_slices.sh ORIGINAL_DIRECTORY --generate   (once: re-pin and re-record)
# The caller supplies the original files; nothing is downloaded and no weights
# are committed. The checker authenticates every byte range it reads.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -lt 1 || $# -gt 2 || ( $# == 2 && $2 != --generate ) ]]; then
    echo "usage: psionic/tests/check_elephant_slices.sh ORIGINAL_DIRECTORY [--generate]" >&2; exit 2
fi
original=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-elephant-slices.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
"$coax" test psionic/tests/elephant-slices-runtime.coax > "$out/checks.log" || { cat "$out/checks.log"; exit 1; }
"$coax" c psionic/tests/elephant-slices-runtime.coax --library --release --header "$out/elephant-slices-runtime.h" > "$out/elephant-slices-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/elephant-slices-runtime.c" psionic/tests/elephant_slices.c -lm -o "$out/elephant-slices"
model="$original/model.safetensors"
if [[ $# == 2 ]]; then
    python3 -I psionic/tests/elephant_slices.py pins "$original" --generate
    python3 -I psionic/tests/elephant_slices.py tokens "$out"
    "$out/elephant-slices" "$model" "$out/tokens.txt" "$out"
    python3 -I psionic/tests/elephant_slices.py record "$out"
fi
python3 -I psionic/tests/elephant_slices.py pins "$original"
python3 -I psionic/tests/elephant_slices.py tokens "$out"
python3 -I psionic/tests/elephant_slices.py inputs "$out"
"$out/elephant-slices" "$model" "$out/tokens.txt" "$out" "$out/full-input.f32" "$out/readout-input.f32"
python3 -I psionic/tests/elephant_slices.py compare "$out" "$original"
echo "Elephant slices: $(( $(date +%s) - start )) s"
