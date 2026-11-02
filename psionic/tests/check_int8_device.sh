#!/usr/bin/env bash
# Chapter 48 on the device: the int8-block-i32-v1 kernels' checks in debug
# and release, CUDA emission, the example transcript, and (with a CUDA driver
# and NVRTC) the kernels on the device against chapter 48's CPU kernels, bit
# for bit, then timed against chapter 37's f32 and BF16 tensor-core kernels.
# Without a device the device step prints SKIP.
# Firm reason: 5 (a thin wrapper: builds the CUDA modules and the C harness and runs them; no decision is made here)
# Usage: psionic/tests/check_int8_device.sh [--mutate]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-int8-device.XXXXXX")
trap 'rm -rf "$out"' EXIT
for unit in int8-kernels int8-device; do
    "$coax" test "psionic/lib/device/$unit.coax"
    "$coax" test "psionic/lib/device/$unit.coax" --release
done
"$coax" cuda psionic/lib/device/int8-kernels.coax --release > "$out/int8.cu"
grep -q 'mma.sync.aligned.m16n8k16' "$out/int8.cu"
"$coax" cuda psionic/lib/device/project.coax --release > "$out/project.cu"
"$coax" cuda psionic/lib/device/warp.coax --release > "$out/warp.cu"
for mode in "" --release; do
    "$coax" run psionic/examples/int8-device.coax $mode > "$out/int8-device.out"
    cmp psionic/fixtures/int8-device.out "$out/int8-device.out"
done
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation)
"$coax" c psionic/lib/device/int8-device.coax --library --release --header "$out/int8-device.h" > "$out/int8-device.c"
"$cc" "${flags[@]}" -I"$out" -Ipsionic/platform "$out/int8-device.c" psionic/platform/cuda-host.c \
    psionic/tests/int8_device.c -ldl -lm -o "$out/int8-device"
"$out/int8-device" "$out/int8.cu" "$out/project.cu" "$out/warp.cu"
if [[ ${1-} == --mutate ]]; then
    for unit in int8-kernels int8-device; do "$coax" mutate "psionic/lib/device/$unit.coax" --root-only; done
fi
echo "chapter 48 device check passes"
