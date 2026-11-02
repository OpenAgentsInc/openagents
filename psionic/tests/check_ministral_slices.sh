#!/usr/bin/env bash
# Chapter 81's original-weight slices of Ministral 3 3B, checkable in about a
# minute: the pinned files' hashes and inventory, Tekken on the fixture
# strings, then the embedding gather, layer 0 and readout rows 0..4095 over
# the chat-format prompt, each against the independent float64 reference in
# psionic/tests/ministral_reference.py within its frozen tolerance.
#   psionic/tests/check_ministral_slices.sh [--run] MODEL_DIRECTORY
# With --run, it instead runs the recorded whole-model run (about ten
# minutes; not a routine check): eight greedy tokens through all 26 layers and
# the cache, the full recompute compared bit for bit, and every layer and the
# last logits against psionic/tests/ministral_run_oracle.py.
# The caller supplies the original files; nothing is downloaded and no weights
# are committed.
set -euo pipefail
cd "$(dirname "$0")/../.."
mode=slices
if [[ ${1-} == --run ]]; then mode=run; shift; fi
if [[ $# -ne 1 ]]; then echo "usage: psionic/tests/check_ministral_slices.sh [--run] MODEL_DIRECTORY" >&2; exit 2; fi
model=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-ministral-slices.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
python3 -I psionic/tests/ministral_reference.py pin "$model"
python3 -I psionic/tests/ministral_reference.py tekken "$model"
python3 -I psionic/tests/ministral_reference.py tiny
if [[ $mode == run ]]; then
    "$coax" c psionic/tests/ministral-run-runtime.coax --library --release --header "$out/ministral-run-runtime.h" > "$out/ministral-run-runtime.c"
    "$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
        -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
        "$out/ministral-run-runtime.c" psionic/tests/ministral_run.c -lm -o "$out/ministral-run"
    python3 -I psionic/tests/ministral_reference.py slice-tokens "$model" "$out"
    "$out/ministral-run" "$model" "$out/tokens.txt" 8 "$out"
    python3 -I psionic/tests/ministral_run_oracle.py "$model" "$out"
    echo "Ministral run: $(( $(date +%s) - start )) s"
    exit 0
fi
# The whole of tekken.json through the Coax reader, the fixture strings encoded.
"$coax" c psionic/tests/ministral-tekken-runtime.coax --library --release --header "$out/ministral-tekken-runtime.h" > "$out/ministral-tekken-runtime.c"
"$cc" -std=c11 -O2 -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/ministral-tekken-runtime.c" psionic/tests/ministral_tekken.c -lm -o "$out/ministral-tekken"
"$out/ministral-tekken" "$model/tekken.json" "$out"
python3 -I psionic/tests/ministral_reference.py tekken-check "$model" "$out"
"$coax" c psionic/tests/ministral-slices-runtime.coax --library --release --header "$out/ministral-slices-runtime.h" > "$out/ministral-slices-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/ministral-slices-runtime.c" psionic/tests/ministral_slices.c -lm -o "$out/ministral-slices"
python3 -I psionic/tests/ministral_reference.py slice-tokens "$model" "$out"
"$out/ministral-slices" "$model/consolidated.safetensors" "$out/tokens.txt" "$out"
python3 -I psionic/tests/ministral_reference.py slices "$model" "$out"
echo "Ministral slices: $(( $(date +%s) - start )) s"
