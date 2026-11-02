#!/usr/bin/env bash
# Chapter 144's original-weight slices of C-RADIOv3-B, in about a minute:
# the header and the inventory of every tensor against the config, the
# stored summary indices and conditioner, then the encoder on a 64 by 48
# image (patches, embedding, block 0, and all 12 blocks to the summary and
# spatial features), the embedding of the 48 by 64 transpose and the patches
# of a 224 by 224 image, each against psionic/tests/radio_slices.py's
# independent binary64 oracle, which reads the same tensors from the file.
#   psionic/tests/check_radio_slices.sh C_RADIO_DIRECTORY
# The caller supplies the original files (MANIFEST in psionic/fixtures/radio);
# nothing is downloaded and no weights are committed.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -ne 1 ]]; then
    echo "usage: psionic/tests/check_radio_slices.sh C_RADIO_DIRECTORY" >&2; exit 2
fi
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-radio-slices.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
"$coax" c psionic/tests/radio-slices-runtime.coax --library --release --header "$out/radio-slices-runtime.h" > "$out/radio-slices-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/radio-slices-runtime.c" psionic/tests/radio_slices.c -lm -o "$out/radio-slices"
"$out/radio-slices" "$1/model.safetensors" "$out/c-radio.f32"
python3 -I psionic/tests/radio_slices.py "$1/config.json" "$1/model.safetensors" "$out/c-radio.f32"
echo "c-radio slices: $(( $(date +%s) - start )) s"
