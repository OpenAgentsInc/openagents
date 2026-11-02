#!/usr/bin/env bash
# Chapter 76: verify a draft block in parallel. Checks in debug and release,
# both example transcripts, and the tiny owned model on the CPU (the device
# programs compiled to C, plain and under ASan/UBSan): greedy parity and
# cache equality after every rollback. With ORIGINAL_DIR and a CUDA device,
# the original gpt-oss-20b: parity and cache checks on the two fixture
# prompts, then one warm-up and RUNS timed runs of each path, and the
# speculation rule's verdict.
# Usage: psionic/tests/check_device_verify.sh [ORIGINAL_DIR [RUNS [K]]]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-device-verify.XXXXXX")
trap 'rm -rf "$out"' EXIT
for mode in "" --release; do
    "$coax" test psionic/lib/gptoss/device-verify.coax $mode
    "$coax" test psionic/lib/speculation-gate.coax $mode
    for example in device-verify device-verify-refuse; do
        "$coax" run "psionic/examples/$example.coax" $mode > "$out/$example.out"
        cmp "psionic/fixtures/device-verify/$example.out" "$out/$example.out"
    done
done
python3 -I psionic/tests/gptoss_fixture.py --header > "$out/gptoss-tiny.h"
"$coax" c psionic/tests/verify-host.coax --library --run-steps 2000000 --header "$out/device-host.h" > "$out/device-host.c"
python3 -I psionic/tests/gptoss_device_bindings.py "$out/device-host.h" > "$out/gptoss-device-cpu.h"
for native in plain sanitized; do
    san=(); if [[ $native == sanitized ]]; then san=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
    "$cc" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -DPSIONIC_DEVICE_TEST "${san[@]}" -I"$out" -Ipsionic/platform \
        "$out/device-host.c" psionic/platform/cuda-host.c psionic/platform/gptoss-device.c psionic/tests/gptoss_verify.c -lm -ldl -o "$out/tiny-$native"
    ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$out/tiny-$native" cpu psionic/fixtures/gptoss/tiny/model.safetensors > "$out/tiny-$native.out"
    cmp psionic/fixtures/device-verify/tiny.out "$out/tiny-$native.out"
done
if [[ $# -lt 1 ]]; then echo "chapter 76 checks pass (no original weights given: the RTX 4080 run not made)"; exit 0; fi
original=$1; runs=${2:-5}; k=${3:-4}
"$coax" cuda psionic/lib/gptoss/device-kernels.coax > "$out/device-kernels.cu"
"$coax" c psionic/tests/verify-host.coax --library --release --run-steps 2000000 --header "$out/device-host.h" > "$out/device-host.c"
"$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -I"$out" -Ipsionic/platform "$out/device-host.c" \
    psionic/platform/cuda-host.c psionic/platform/gptoss-device.c psionic/tests/gptoss_verify.c -lm -ldl -o "$out/verify"
build="coax-$(sha256sum "$coax" | cut -c1-12)-cuda-$(sha256sum "$out/device-kernels.cu" | cut -c1-16)-host-$(sha256sum "$out/device-host.c" | cut -c1-16)"
commit=$(git rev-parse --short=10 HEAD)
if ! git diff --quiet HEAD -- psionic; then commit="$commit+uncommitted"; fi
machine="$(hostname) $(uname -sr) driver $(nvidia-smi --query-gpu=driver_version --format=csv,noheader 2>/dev/null || echo unknown)"
echo "load average before: $(cut -d' ' -f1-3 /proc/loadavg)"
status=0
"$out/verify" cuda "$out/device-kernels.cu" "$original" "$build" "$commit" "$machine" "$runs" "$k" || status=$?
echo "load average after: $(cut -d' ' -f1-3 /proc/loadavg)"
exit $status
