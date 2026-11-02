#!/usr/bin/env bash
# Chapter 163's whole-model run of Nemotron 3 Nano 4B on the CPU, recorded
# as a fixture (psionic/fixtures/nemotron-run.txt) rather than run as a
# routine check: about 2 minutes and 22 GB for the Coax run, then the oracle.
# Firm reason: 5 (a thin wrapper: builds a Coax tool with the host C compiler, then runs it and its oracles)
#   psionic/tests/check_nemotron_run.sh MODEL_DIRECTORY [STEPS]
# The tokenizer encodes the fixture texts over the full tokenizer.json and
# the prompt; the model runs the prompt layer by layer, then greedy decoding
# from the cache, then the whole sequence again from an empty cache. The
# Python oracle (nemotron_tokens.py check, nemotron_slices.py model) checks
# the ids and recomputes the whole model in binary64.
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ $# -lt 1 || $# -gt 2 ]]; then echo "usage: psionic/tests/check_nemotron_run.sh MODEL_DIRECTORY [STEPS]" >&2; exit 2; fi
model=$1
steps=${2:-8}
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-nemotron-run.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
python3 -I psionic/tests/nemotron_manifest.py "$model"
"$coax" c psionic/tests/nemotron-run-runtime.coax --library --release --header "$out/nemotron-run-runtime.h" > "$out/nemotron-run-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/nemotron-run-runtime.c" psionic/tests/nemotron_run.c -lm -o "$out/nemotron-run"
python3 -I psionic/tests/nemotron_slices.py ranges "$model" "$out"
python3 -I psionic/tests/nemotron_tokens.py texts "$out/texts.bin"
"$out/nemotron-run" "$model" "$out/ranges.txt" "$out/texts.bin" "$out" "$steps"
python3 -I psionic/tests/nemotron_tokens.py check "$model" "$out/coax-ids.txt"
python3 -I psionic/tests/nemotron_slices.py model "$model" "$out"
echo "Nemotron whole-model run: $(( $(date +%s) - start )) s"
