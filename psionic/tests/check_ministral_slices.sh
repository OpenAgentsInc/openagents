#!/usr/bin/env bash
# Chapter 81's original-weight slices of Ministral 3 3B, checkable in about a
# minute: the pinned files' hashes and inventory, Tekken on the fixture
# strings, then the embedding gather, layer 0 and readout rows 0..4095 over
# the chat-format prompt, each against the independent float64 reference in
# psionic/tests/ministral_reference.py within its frozen tolerance.
#   psionic/tests/check_ministral_slices.sh MODEL_DIRECTORY
# The caller supplies the original files; nothing is downloaded and no weights
# are committed.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -ne 1 ]]; then echo "usage: psionic/tests/check_ministral_slices.sh MODEL_DIRECTORY" >&2; exit 2; fi
model=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-ministral-slices.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
python3 -I psionic/tests/ministral_reference.py pin "$model"
python3 -I psionic/tests/ministral_reference.py tekken "$model"
python3 -I psionic/tests/ministral_reference.py tiny
"$coax" c psionic/tests/ministral-slices-runtime.coax --library --release --header "$out/ministral-slices-runtime.h" > "$out/ministral-slices-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/ministral-slices-runtime.c" psionic/tests/ministral_slices.c -lm -o "$out/ministral-slices"
python3 -I psionic/tests/ministral_reference.py slice-tokens "$model" "$out"
"$out/ministral-slices" "$model/consolidated.safetensors" "$out/tokens.txt" "$out"
python3 -I psionic/tests/ministral_reference.py slices "$model" "$out"
echo "Ministral slices: $(( $(date +%s) - start )) s"
