#!/usr/bin/env bash
# Chapter 129: the tiny decoder trained on one GPU. Checks in debug and
# release, CUDA emission of the step's kernels, the example transcripts, and
# (with a CUDA driver and NVRTC) the run on the device: the plan against the
# hand count, the fixed-order pieces bit for bit, the run against chapter
# 118's CPU run within the frozen tolerance, resume bit for bit, and a speed
# receipt. Without a device the device step prints SKIP.
# Usage: psionic/tests/check_gpu_train.sh [--mutate] [STEPS]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mutate=0
if [[ ${1:-} == --mutate ]]; then mutate=1; shift; fi
steps=${1:-20}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-gpu-train.XXXXXX")
trap 'rm -rf "$out"' EXIT
for unit in train-kernels gpu-train; do
    "$coax" test "psionic/lib/transformer/$unit.coax"
    "$coax" test "psionic/lib/transformer/$unit.coax" --release
done
"$coax" cuda psionic/lib/transformer/train-kernels.coax --release > "$out/train-kernels.cu"
for name in gpu-train gpu-train-refuse; do
    for mode in "" --release; do
        "$coax" run "psionic/examples/$name.coax" $mode > "$out/$name.out"
        cmp "psionic/fixtures/$name.out" "$out/$name.out"
    done
done
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation)
"$coax" c psionic/lib/transformer/gpu-train.coax --library --release --header "$out/gpu-train.h" > "$out/gpu-train.c"
"$cc" "${flags[@]}" -I"$out" -Ipsionic/platform "$out/gpu-train.c" psionic/platform/cuda-host.c \
    psionic/tests/gpu_train_device.c -ldl -lm -o "$out/gpu-train-device"
"$out/gpu-train-device" "$out/train-kernels.cu" "$steps"
if [[ $mutate == 1 ]]; then
    for unit in train-kernels gpu-train; do "$coax" mutate "psionic/lib/transformer/$unit.coax" --root-only; done
fi
echo "chapter 129 passes"
