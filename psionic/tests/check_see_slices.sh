#!/usr/bin/env bash
# Chapter 143's original-weight slices of the vision tower, in about a minute:
# the inventory of every model.visual tensor against the config, then the
# encoder (patch embedding with positions, block 0, the merger) on a 2 by 4
# patch grid and the processor's patches for a 320 by 256 image, each against
# psionic/tests/see_slices.py's independent binary64 oracle, which reads the
# same tensors from the file itself.
#   psionic/tests/check_see_slices.sh ELEPHANT_DIRECTORY [MAMMOTH_DIRECTORY]
# The caller supplies the original files; nothing is downloaded and no weights
# are committed.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -lt 1 || $# -gt 2 ]]; then
    echo "usage: psionic/tests/check_see_slices.sh ELEPHANT_DIRECTORY [MAMMOTH_DIRECTORY]" >&2; exit 2
fi
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-see-slices.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
"$coax" c psionic/tests/see-slices-runtime.coax --library --release --header "$out/see-slices-runtime.h" > "$out/see-slices-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/see-slices-runtime.c" psionic/tests/see_slices.c -lm -o "$out/see-slices"
"$out/see-slices" "$1/model.safetensors" elephant "$out/elephant.f32"
python3 -I psionic/tests/see_slices.py "$1/config.json" "$1/model.safetensors" "$out/elephant.f32"
if [[ $# == 2 ]]; then
    shard="$2/model-00002-of-00002.safetensors"
    "$out/see-slices" "$shard" mammoth "$out/mammoth.f32"
    python3 -I psionic/tests/see_slices.py "$2/config.json" "$shard" "$out/mammoth.f32"
fi
echo "vision slices: $(( $(date +%s) - start )) s"
