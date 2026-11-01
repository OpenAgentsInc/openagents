#!/usr/bin/env bash
# Chapter 131: train the same model eager and compiled. Checks in debug and
# release, the example transcript in both modes, then the bench: the three
# paths timed on this machine's monotonic clock, compared, and written as a
# receipt (JSON, psionic.train-compare.v1) to RECEIPT, default $TMPDIR. The
# bench exits nonzero unless the paths agree bit for bit and both
# comparisons are admitted. Times are a receipt, never a transcript.
# Usage: psionic/tests/check_trainer_compiled.sh [--bench-only] [RECEIPT]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
bench_only=0
if [[ ${1:-} == --bench-only ]]; then bench_only=1; shift; fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-trainer-compiled.XXXXXX")
trap 'rm -rf "$out"' EXIT
receipt=${1:-$out/receipt.json}
if [[ $bench_only == 0 ]]; then
    for unit in train-compare trainer-compiled; do
        "$coax" test "psionic/lib/$unit.coax"
        "$coax" test "psionic/lib/$unit.coax" --release
    done
    for mode in "" --release; do
        "$coax" run psionic/examples/eager-and-compiled.coax $mode > "$out/eager-and-compiled.out"
        cmp psionic/fixtures/eager-and-compiled.out "$out/eager-and-compiled.out"
    done
fi
"$coax" c psionic/tests/trainer-compiled-bench.coax --release > "$out/bench.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -w "$out/bench.c" psionic/tests/trainer_compiled_clock.c -lm -o "$out/bench"
"$out/bench" > "$receipt"
cat "$receipt"
echo "chapter 131 passes"
