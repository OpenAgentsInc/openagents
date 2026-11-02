#!/usr/bin/env bash
# Chapter 175: continuous batching of gpt-oss-20b on the device.
# Firm reason: 5 (a thin wrapper: builds the CUDA module and the C harness and runs them; no decision is made here)
# Checks the batched-step unit in debug and release; with ORIGINAL_DIR and a
# CUDA device, the harness on the original gpt-oss-20b: every request alone,
# then batched at concurrency 1, 4 and 8, one warm-up and RUNS timed runs each.
# Usage: psionic/tests/check_device_batch.sh [ORIGINAL_DIR [RUNS [HI_WORD LIST_WORD]]]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-device-batch.XXXXXX")
trap 'rm -rf "$out"' EXIT
for mode in "" --release; do "$coax" test psionic/lib/gptoss/device-batch.coax $mode; done
if [[ $# -lt 1 ]]; then echo "chapter 175 device checks pass (no original weights given: the RTX 4080 run not made)"; exit 0; fi
original=$1; runs=${2:-5}; words=()
if [[ $# -ge 4 ]]; then words=("$3" "$4"); fi
"$coax" cuda psionic/lib/gptoss/device-kernels.coax > "$out/device-kernels.cu"
"$coax" c psionic/tests/batch-host.coax --library --release --run-steps 2000000 --header "$out/device-host.h" > "$out/device-host.c"
"$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -I"$out" -Ipsionic/platform "$out/device-host.c" \
    psionic/platform/cuda-host.c psionic/platform/gptoss-device.c psionic/tests/gptoss_batch.c -lm -ldl -o "$out/batch"
build="coax-$(sha256sum "$coax" | cut -c1-12)-cuda-$(sha256sum "$out/device-kernels.cu" | cut -c1-16)-host-$(sha256sum "$out/device-host.c" | cut -c1-16)"
commit=$(git rev-parse --short=10 HEAD)
if ! git diff --quiet HEAD -- psionic; then commit="$commit+uncommitted"; fi
machine="$(hostname) $(uname -sr) driver $(nvidia-smi --query-gpu=driver_version --format=csv,noheader 2>/dev/null || echo unknown)"
echo "load average before: $(cut -d' ' -f1-3 /proc/loadavg)"
status=0
"$out/batch" "$out/device-kernels.cu" "$original" "$build" "$commit" "$machine" "$runs" "${words[@]}" || status=$?
echo "load average after: $(cut -d' ' -f1-3 /proc/loadavg)"
exit $status
