#!/usr/bin/env bash
# Chapter 102: choose the draft length by expected gain. Checks in debug and
# release and both example transcripts. With ORIGINAL_DIR and a CUDA device,
# the original gpt-oss-20b: a calibration pass fits the cost line and the
# acceptance model, then RUNS timed runs of target only, fixed k = 1..4 and
# adaptive k, each compared by chapter 76's rule.
# Usage: psionic/tests/check_draft_length.sh [ORIGINAL_DIR [RUNS]]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-draft-length.XXXXXX")
trap 'rm -rf "$out"' EXIT
for mode in "" --release; do
    "$coax" test psionic/lib/draft-length.coax $mode
    "$coax" test psionic/lib/draft-length-run.coax $mode
    for example in draft-length draft-length-refuse; do
        "$coax" run "psionic/examples/$example.coax" $mode > "$out/$example.out"
        cmp "psionic/fixtures/draft-length/$example.out" "$out/$example.out"
    done
done
if [[ $# -lt 1 ]]; then echo "chapter 102 checks pass (no original weights given: the RTX 4080 run not made)"; exit 0; fi
original=$1; runs=${2:-5}
"$coax" cuda psionic/lib/gptoss/device-kernels.coax > "$out/device-kernels.cu"
"$coax" c psionic/tests/draft-length-host.coax --library --release --run-steps 2000000 --header "$out/device-host.h" > "$out/device-host.c"
"$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -I"$out" -Ipsionic/platform "$out/device-host.c" \
    psionic/platform/cuda-host.c psionic/platform/gptoss-device.c psionic/tests/gptoss_draft_length.c -lm -ldl -o "$out/draft-length"
build="coax-$(sha256sum "$coax" | cut -c1-12)-cuda-$(sha256sum "$out/device-kernels.cu" | cut -c1-16)-host-$(sha256sum "$out/device-host.c" | cut -c1-16)"
commit=$(git rev-parse --short=10 HEAD)
if ! git diff --quiet HEAD -- psionic; then commit="$commit+uncommitted"; fi
machine="$(hostname) $(uname -sr) driver $(nvidia-smi --query-gpu=driver_version --format=csv,noheader 2>/dev/null || echo unknown)"
echo "load average before: $(cut -d' ' -f1-3 /proc/loadavg)"
status=0
"$out/draft-length" cuda "$out/device-kernels.cu" "$original" "$build" "$commit" "$machine" "$runs" || status=$?
echo "load average after: $(cut -d' ' -f1-3 /proc/loadavg)"
exit $status
