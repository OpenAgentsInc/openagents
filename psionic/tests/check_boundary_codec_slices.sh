#!/usr/bin/env bash
# Chapter 193's original-weight slice of Ministral 3 3B, about a minute: the
# pinned file's hashes, then the residual rows after layers 0 and 1 over the
# chat-format prompt, framed by every codec of boundary-codec.coax and
# received bit for bit, with bytes, timings and chapter 182's link costs.
#   psionic/tests/check_boundary_codec_slices.sh MODEL_DIRECTORY [FIXTURE.coax]
# With FIXTURE.coax it also writes the checks' real rows there. The caller
# supplies the original files; nothing is downloaded and no weights are
# committed.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -lt 1 || $# -gt 2 ]]; then echo "usage: psionic/tests/check_boundary_codec_slices.sh MODEL_DIRECTORY [FIXTURE.coax]" >&2; exit 2; fi
model=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-codec-slices.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
python3 -I psionic/tests/ministral_reference.py pin "$model"
"$coax" c psionic/tests/boundary-codec-slices-runtime.coax --library --release --header "$out/boundary-codec-slices-runtime.h" > "$out/boundary-codec-slices-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/boundary-codec-slices-runtime.c" psionic/tests/boundary_codec_slices.c -lm -o "$out/codec-slices"
python3 -I psionic/tests/ministral_reference.py slice-tokens "$model" "$out"
"$out/codec-slices" "$model/consolidated.safetensors" "$out/tokens.txt" ${2+"$2"}
echo "Boundary codec slices: $(( $(date +%s) - start )) s"
