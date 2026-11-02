#!/usr/bin/env bash
# Chapter 187's original-weight check of stage packages on Ministral 3 3B,
# about five minutes: the pinned file's hashes; both stages of the 14 + 12
# split built, written out and verified; chapter 186's two-layer slice run
# from packaged stages and compared bit for bit with the whole; one changed
# byte refused.
#   psionic/tests/check_stage_packages.sh MODEL_DIRECTORY
# The caller supplies the original files; nothing is downloaded, no weight
# byte is copied, and the manifests go to a temporary directory.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -ne 1 ]]; then echo "usage: psionic/tests/check_stage_packages.sh MODEL_DIRECTORY" >&2; exit 2; fi
model=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-stage-packages.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
python3 -I psionic/tests/ministral_reference.py pin "$model"
"$coax" c psionic/tests/stage-packages-runtime.coax --library --release --header "$out/stage-packages-runtime.h" > "$out/stage-packages-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/stage-packages-runtime.c" psionic/tests/stage_packages.c -lm -o "$out/stage-packages"
python3 -I psionic/tests/ministral_reference.py slice-tokens "$model" "$out"
"$out/stage-packages" "$model/consolidated.safetensors" "$out/tokens.txt" "$out"
echo "Stage packages: $(( $(date +%s) - start )) s"
