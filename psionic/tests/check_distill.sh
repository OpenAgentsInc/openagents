#!/usr/bin/env bash
# Chapter 223: distill a teacher into a small router. Checks in debug and
# release, both example transcripts, and the latency harness: the distilled
# student on chapter 222's frozen 512-token workload, RUNS interleaved warm
# pairs on this CPU. The harness writes a receipt (JSON) to RECEIPT, default
# $TMPDIR; times there are measurements, never a transcript.
# Usage: psionic/tests/check_distill.sh [RUNS [RECEIPT]]
# Exit status: the harness's: 0 when the 20 ms P99 gate is met, 1 when it is
# missed, 5 when it refuses.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
runs=${1:-200}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-distill.XXXXXX")
trap 'rm -rf "$out"' EXIT
receipt=${2:-$out/receipt.json}
for mode in "" --release; do
    "$coax" test psionic/lib/distill.coax $mode
    for name in distill-router distill-router-refuse; do
        "$coax" run "psionic/examples/$name.coax" $mode > "$out/$name.out"
        cmp "psionic/fixtures/distill/$name.out" "$out/$name.out"
    done
done
"$coax" c psionic/lib/distill.coax --library --release --header "$out/distill.h" > "$out/distill.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$out" "$out/distill.c" psionic/tests/distill_bench.c -lm -o "$out/bench"
build="coax-$(sha256sum "$coax" | cut -c1-12)-c-$(sha256sum "$out/distill.c" | cut -c1-16)-$("$cc" --version | head -1 | tr -d '"')"
commit=$(git rev-parse --short=10 HEAD)
if ! git diff --quiet HEAD -- psionic; then commit="$commit+uncommitted"; fi
cpu=$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ *//')
machine="$(hostname) $(uname -sr) $cpu, load $(cut -d' ' -f1-3 /proc/loadavg)"
status=0
"$out/bench" "$receipt" "$runs" "$build" "$commit" "$machine" || status=$?
echo "receipt: $receipt"
exit $status
