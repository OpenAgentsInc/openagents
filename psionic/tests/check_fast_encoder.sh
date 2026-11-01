#!/usr/bin/env bash
# Chapter 222: a decision in milliseconds. Checks in debug and release, both
# example transcripts, and the latency harness on this CPU: COLD fresh
# processes per engine (one decision each), then RUNS interleaved warm pairs
# at 512 token IDs. The harness writes a receipt (JSON) to RECEIPT, default
# $TMPDIR; times there are measurements, never a transcript.
# Usage: psionic/tests/check_fast_encoder.sh [RUNS [COLD [RECEIPT]]]
# Exit status: the harness's: 0 when the 20 ms P99 gate is met, 1 when it is
# missed, 5 when it refuses.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
runs=${1:-200}
cold=${2:-20}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-fast-encoder.XXXXXX")
trap 'rm -rf "$out"' EXIT
receipt=${3:-$out/receipt.json}
for mode in "" --release; do
    "$coax" test psionic/lib/fast-encoder.coax $mode
    for name in fast-decision fast-decision-refuse; do
        "$coax" run "psionic/examples/$name.coax" $mode > "$out/$name.out"
        cmp "psionic/fixtures/fast-encoder/$name.out" "$out/$name.out"
    done
done
"$coax" c psionic/lib/fast-encoder.coax --library --release --header "$out/fast-encoder.h" > "$out/fast-encoder.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$out" "$out/fast-encoder.c" psionic/tests/fast_encoder_bench.c -lm -o "$out/bench"
build="coax-$(sha256sum "$coax" | cut -c1-12)-c-$(sha256sum "$out/fast-encoder.c" | cut -c1-16)-$("$cc" --version | head -1 | tr -d '"')"
commit=$(git rev-parse --short=10 HEAD)
if ! git diff --quiet HEAD -- psionic; then commit="$commit+uncommitted"; fi
cpu=$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ *//')
machine="$(hostname) $(uname -sr) $cpu, load $(cut -d' ' -f1-3 /proc/loadavg)"
: > "$out/cold-q8"; : > "$out/cold-ref"
for i in $(seq 1 "$cold"); do
    "$out/bench" cold q8 >> "$out/cold-q8"
    "$out/bench" cold reference >> "$out/cold-ref"
done
status=0
"$out/bench" warm "$receipt" "$runs" "$out/cold-q8" "$out/cold-ref" "$build" "$commit" "$machine" || status=$?
echo "receipt: $receipt"
exit $status
