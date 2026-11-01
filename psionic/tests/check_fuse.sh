#!/usr/bin/env bash
# Chapter 43: fusion admitted only for non-overlapping spans, and steps
# recorded once and replayed as device graphs. Checks in debug and release,
# CUDA emission, the example transcripts, and (with a CUDA driver and NVRTC)
# the step on the device: unfused, fused and replayed, each against the C
# build bit for bit. Without a device the device step prints SKIP.
# Usage: psionic/tests/check_fuse.sh [--mutate]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-fuse.XXXXXX")
trap 'rm -rf "$out"' EXIT
for unit in fuse-kernels fuse; do
    "$coax" test "psionic/lib/device/$unit.coax"
    "$coax" test "psionic/lib/device/$unit.coax" --release
done
"$coax" cuda psionic/lib/device/fuse-kernels.coax --release > "$out/fuse.cu"
for name in fuse fuse-refuse; do
    for mode in "" --release; do
        "$coax" run "psionic/examples/$name.coax" $mode > "$out/$name.out"
        cmp "psionic/fixtures/$name.out" "$out/$name.out"
    done
done
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation)
"$coax" c psionic/lib/device/fuse.coax --library --release --header "$out/fuse.h" > "$out/fuse.c"
"$cc" "${flags[@]}" -I"$out" -Ipsionic/platform "$out/fuse.c" psionic/platform/cuda-host.c \
    psionic/tests/fuse_device.c -ldl -lm -o "$out/fuse-device"
"$out/fuse-device" "$out/fuse.cu"
if [[ ${1:-} == --mutate ]]; then
    for unit in fuse-kernels fuse; do "$coax" mutate "psionic/lib/device/$unit.coax"; done
fi
echo "chapter 43 passes"
