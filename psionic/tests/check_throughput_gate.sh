#!/usr/bin/env bash
# Chapter 71: the throughput gate. Checks in debug and release and both
# example transcripts; with ORIGINAL_DIR and a CUDA device, the gate itself on
# the original gpt-oss-20b (one warm-up and RUNS timed runs, default 5).
# Usage: psionic/tests/check_throughput_gate.sh [ORIGINAL_DIR [RUNS [HI_WORD LIST_WORD [SLICES]]]]
# ORIGINAL_DIR holds the pinned original files that chapter 65 authenticates.
# HI_WORD and LIST_WORD are the reference path's greedy-output words; SLICES
# turns on the tensor-core prefill expert products (gptoss/device-mma.coax).
# Exit status: 0 when the gate passes, 1 when it fails, 4 when it is too
# noisy, 5 when it refuses.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-throughput-gate.XXXXXX")
trap 'rm -rf "$out"' EXIT
for mode in "" --release; do
    "$coax" test psionic/lib/throughput-gate.coax $mode
    for example in throughput-gate throughput-gate-refuse; do
        "$coax" run "psionic/examples/$example.coax" $mode > "$out/$example.out"
        cmp "psionic/fixtures/throughput-gate/$example.out" "$out/$example.out"
    done
done
if [[ $# -lt 1 ]]; then echo "chapter 71 checks pass (no original weights given: gate not run)"; exit 0; fi
original=$1; runs=${2:-5}; words=(); slices=""
if [[ $# -ge 4 ]]; then words=("$3" "$4"); fi
if [[ $# -ge 5 ]]; then slices=$5; fi
"$coax" cuda psionic/lib/gptoss/device-kernels.coax > "$out/device-kernels.cu"
"$coax" c psionic/tests/gate-host.coax --library --release --run-steps 2000000 --header "$out/device-host.h" > "$out/device-host.c"
"$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -I"$out" -Ipsionic/platform "$out/device-host.c" \
    psionic/platform/cuda-host.c psionic/platform/gptoss-device.c psionic/tests/gptoss_gate.c -lm -ldl -o "$out/gate"
build="coax-$(sha256sum "$coax" | cut -c1-12)-cuda-$(sha256sum "$out/device-kernels.cu" | cut -c1-16)"
mma=()
if [[ -n $slices ]]; then
    "$coax" cuda psionic/lib/gptoss/device-mma.coax --release > "$out/device-mma.cu"
    build="$build-mma-$(sha256sum "$out/device-mma.cu" | cut -c1-16)"
    mma=("$out/device-mma.cu" "$slices")
fi
build="$build-host-$(sha256sum "$out/device-host.c" | cut -c1-16)"
commit=$(git rev-parse --short=10 HEAD)
if ! git diff --quiet HEAD -- psionic; then commit="$commit+uncommitted"; fi
machine="$(hostname) $(uname -sr) driver $(nvidia-smi --query-gpu=driver_version --format=csv,noheader 2>/dev/null || echo unknown)"
echo "load average before: $(cut -d' ' -f1-3 /proc/loadavg)"
status=0
"$out/gate" "$out/device-kernels.cu" "$original" "$build" "$commit" "$machine" "$runs" "${words[@]}" "${mma[@]}" || status=$?
echo "load average after: $(cut -d' ' -f1-3 /proc/loadavg)"
exit $status
