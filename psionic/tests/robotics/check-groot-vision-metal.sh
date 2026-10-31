#!/usr/bin/env bash
# Thin effects wrapper for actual original vision24 on required Metal GPU.
set -euo pipefail
cd "$(dirname "$0")/../../.."
shards="${1:?usage: check-groot-vision-metal.sh ORIGINAL_SHARDS ORIGINAL_SDK_CAPSULE NEW_OUTPUT_DIRECTORY}"
capsule="${2:?usage: check-groot-vision-metal.sh ORIGINAL_SHARDS ORIGINAL_SDK_CAPSULE NEW_OUTPUT_DIRECTORY}"
work="${3:?usage: check-groot-vision-metal.sh ORIGINAL_SHARDS ORIGINAL_SDK_CAPSULE NEW_OUTPUT_DIRECTORY}"
if [[ -e "$work" ]];then echo "vision output directory already exists: $work" >&2;exit 2;fi
mkdir -p "$work"
coax="${COAX:-$PWD/coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}";python="${REFERENCE_PYTHON:-${GROOT_REFERENCE_PYTHON:-python3}}"
"$python" -I psionic/tests/robotics/groot_vision_metal_reference.py prepare "$capsule" "$work" --shards "$shards"
"$coax" c psionic/lib/robotics/groot-vision-metal-plan.coax --release --library --header "$work/vision-plan.h" > "$work/plan.c"
"$coax" metal-compute psionic/lib/device/tile.coax --release > "$work/tile.metal"
"$coax" metal-compute psionic/lib/robotics/groot-device-kernels.coax --release > "$work/device.metal"
"$coax" metal-compute psionic/lib/robotics/groot-vision-device.coax --release > "$work/vision.metal"
"$coax" metal-compute psionic/lib/robotics/groot-action-wide-kernels.coax --release > "$work/wide.metal"
"$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -ffp-contract=off -fno-fast-math -I psionic/platform -c psionic/platform/metal-host.m -o "$work/metal-host.o"
"$cc" -std=c11 -O2 -fblocks -mmacosx-version-min=15.0 -ffp-contract=off -fno-fast-math -I"$work" -I psionic/platform -I psionic/tests/robotics "$work/plan.c" psionic/tests/robotics/groot_vision_metal.c "$work/metal-host.o" -framework Metal -framework Foundation -lm -o "$work/vision-metal"
"$work/vision-metal" "$shards" "$work/pixels.f32" "$work/tile.metal" "$work/device.metal" "$work/vision.metal" "$work/wide.metal" "$work" "$work/device.txt"
"$python" -I psionic/tests/robotics/groot_vision_metal_reference.py compare "$capsule" "$work"
