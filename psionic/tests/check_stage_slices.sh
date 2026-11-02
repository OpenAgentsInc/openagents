#!/usr/bin/env bash
# Chapter 186's original-weight slice of Ministral 3 3B, about a minute:
# the pinned file's hashes, then layers 0 and 1 and the full readout over
# the chat-format prompt, whole and split into two stages, compared bit for
# bit, and a stage holding another artifact's digest refusing the boundary.
#   psionic/tests/check_stage_slices.sh MODEL_DIRECTORY
# The caller supplies the original files; nothing is downloaded and no weights
# are committed.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -ne 1 ]]; then echo "usage: psionic/tests/check_stage_slices.sh MODEL_DIRECTORY" >&2; exit 2; fi
model=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-stage-slices.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
python3 -I psionic/tests/ministral_reference.py pin "$model"
"$coax" c psionic/tests/stage-slices-runtime.coax --library --release --header "$out/stage-slices-runtime.h" > "$out/stage-slices-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/stage-slices-runtime.c" psionic/tests/stage_slices.c -lm -o "$out/stage-slices"
python3 -I psionic/tests/ministral_reference.py slice-tokens "$model" "$out"
"$out/stage-slices" "$model/consolidated.safetensors" "$out/tokens.txt"
echo "Stage slices: $(( $(date +%s) - start )) s"
