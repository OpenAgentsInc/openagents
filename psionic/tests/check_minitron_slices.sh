#!/usr/bin/env bash
# Chapter 58's original-weight slices of Llama-3.1-Minitron-4B-Width-Base,
# checkable in about a minute: the pinned files' hashes and inventory, the
# Llama 3 tokenizer on the fixture strings, then the embedding gather, layer
# 0 over the prompt (in one call and through the key-value cache, compared
# bit for bit) and lm_head rows 0..4095, each against the independent float64
# reference in psionic/tests/minitron_reference.py within its frozen
# tolerance.
#   psionic/tests/check_minitron_slices.sh MODEL_DIRECTORY
# With --continue, it instead runs the recorded whole-model continuations
# (minutes; not a routine check).
# The caller supplies the original files; nothing is downloaded and no
# weights are committed.
set -euo pipefail
cd "$(dirname "$0")/../.."
mode=slices
if [[ ${1-} == --continue ]]; then mode=continue; shift; fi
if [[ $# -ne 1 ]]; then echo "usage: psionic/tests/check_minitron_slices.sh [--continue] MODEL_DIRECTORY" >&2; exit 2; fi
model=$1
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-minitron.XXXXXX")
trap 'rm -rf "$out"' EXIT
start=$(date +%s)
python3 -I psionic/tests/minitron_reference.py pin "$model"
python3 -I psionic/tests/minitron_reference.py tokens "$model"
python3 -I psionic/tests/minitron_reference.py tiny
"$coax" c psionic/tests/minitron-slices-runtime.coax --library --release --header "$out/minitron-slices-runtime.h" > "$out/minitron-slices-runtime.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror \
    -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -I"$out" \
    "$out/minitron-slices-runtime.c" psionic/tests/minitron_run.c -lm -o "$out/minitron-run"
python3 -I psionic/tests/minitron_reference.py slice-tokens "$model" "$out"
if [[ $mode == slices ]]; then
    "$out/minitron-run" slices "$model" "$out/tokens.txt" "$out"
    python3 -I psionic/tests/minitron_reference.py slices "$model" "$out"
else
    # Greedy with the full-recompute comparison, and seeded at temperature 1
    # with seed 58, side by side.
    "$out/minitron-run" continue "$model" "$out/tokens.txt" 8 0 0 full > "$out/greedy.txt" &
    "$out/minitron-run" continue "$model" "$out/tokens.txt" 8 1 58 > "$out/seeded.txt" &
    wait %1 && wait %2
    cat "$out/greedy.txt" "$out/seeded.txt"
fi
echo "Minitron $mode: $(( $(date +%s) - start )) s"
