#!/usr/bin/env bash
# Chapter 163's original-weight slices of Nemotron 3 Nano 4B: the embedding
# rows of a six-token prompt, Mamba-2 layer 0 (whole, resumed, stepped), MLP
# layer 1, attention layer 12 (whole and from the cache) and readout rows
# 0..8191, each against an independent binary64 Python oracle.
#   psionic/tests/check_nemotron_slices.sh MODEL_DIRECTORY
# The caller supplies the pinned download; nothing is downloaded and no
# weights are committed. The manifest check hashes the whole file first.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -ne 1 ]]; then echo "usage: psionic/tests/check_nemotron_slices.sh MODEL_DIRECTORY" >&2; exit 2; fi
model=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-nemotron-slices.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
python3 -I psionic/tests/nemotron_manifest.py "$model"
"$coax" c psionic/tests/nemotron-slices-runtime.coax --library --release --header "$out/nemotron-slices-runtime.h" > "$out/nemotron-slices-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/nemotron-slices-runtime.c" psionic/tests/nemotron_slices.c -lm -o "$out/nemotron-slices"
python3 -I psionic/tests/nemotron_slices.py ranges "$model" "$out"
"$out/nemotron-slices" "$model/model.safetensors" "$out/ranges.txt" "$out/tokens.txt" "$out"
python3 -I psionic/tests/nemotron_slices.py compare "$model" "$out"
echo "Nemotron slices: $(( $(date +%s) - start )) s"
