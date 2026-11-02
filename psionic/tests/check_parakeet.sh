#!/usr/bin/env bash
# Chapter 153's original-weight run of nvidia/parakeet-tdt-0.6b-v3 on the CPU:
# the pinned file's hash and inventory, then the four evaluation clips in
# psionic/fixtures/parakeet transcribed (one process each) and scored against
# the frozen thresholds, then the log-mel features, the subsampling and
# encoder layers 0 and 1 of the shortest clip against the independent Python
# oracle. About ten minutes on 28 cores.
#   psionic/tests/check_parakeet.sh MODEL_DIRECTORY
# The caller supplies the pinned download; nothing is downloaded and no
# weights are committed.
# Firm reason: 5 (a thin wrapper around the Coax compiler, the C compiler and the oracle).
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -ne 1 ]]; then echo "usage: psionic/tests/check_parakeet.sh MODEL_DIRECTORY" >&2; exit 2; fi
model=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-parakeet.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
python3 -I psionic/tests/parakeet_manifest.py "$model"
"$coax" c psionic/tests/parakeet-run-runtime.coax --library --release --header "$out/parakeet-run-runtime.h" > "$out/parakeet-run-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/parakeet-run-runtime.c" psionic/tests/parakeet_run.c -lm -o "$out/parakeet-run"
python3 -I psionic/tests/parakeet_oracle.py ranges "$model" "$out"
"$out/parakeet-run" "$model" "$out/ranges.txt" psionic/fixtures/parakeet "$out" 6930-75918-0011
python3 -I psionic/tests/parakeet_oracle.py compare "$model" "$out" psionic/fixtures/parakeet/6930-75918-0011.pcm
echo "Parakeet: $(( $(date +%s) - start )) s"
