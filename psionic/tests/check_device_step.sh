#!/usr/bin/env bash
# Chapter 99: a decode step with no host waits. Checks in debug and release
# and both example transcripts; with ORIGINAL_DIR and a CUDA device, the
# recorded step on the original gpt-oss-20b: greedy and seeded outputs
# against the host paths, host waits counted, one warm-up and RUNS timed runs.
# Usage: psionic/tests/check_device_step.sh [ORIGINAL_DIR [RUNS]]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-device-step.XXXXXX")
trap 'rm -rf "$out"' EXIT
for mode in "" --release; do
    "$coax" test psionic/lib/gptoss/device-sample.coax $mode
    "$coax" test psionic/lib/gptoss/device-step.coax $mode
    for example in device-step device-step-refuse; do
        status=0; "$coax" run "psionic/examples/$example.coax" $mode > "$out/$example.out" || status=$?
        cmp "psionic/fixtures/device-step/$example.out" "$out/$example.out"
        if [[ $status -ne 0 ]]; then echo "$example: exit status $status"; exit 1; fi
    done
done
if [[ $# -lt 1 ]]; then echo "chapter 99 checks pass (no original weights given: the RTX 4080 run not made)"; exit 0; fi
original=$1; runs=${2:-5}
"$coax" cuda psionic/lib/gptoss/device-sample.coax > "$out/device-step.cu"
"$coax" c psionic/tests/step-host.coax --library --release --run-steps 2000000 --header "$out/device-host.h" > "$out/device-host.c"
"$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -I"$out" -Ipsionic/platform "$out/device-host.c" \
    psionic/platform/cuda-host.c psionic/platform/gptoss-device.c psionic/tests/gptoss_step.c -lm -ldl -o "$out/step"
echo "build: coax-$(sha256sum "$coax" | cut -c1-12)-cuda-$(sha256sum "$out/device-step.cu" | cut -c1-16)-host-$(sha256sum "$out/device-host.c" | cut -c1-16)"
"$out/step" "$out/device-step.cu" "$original" "$runs"
