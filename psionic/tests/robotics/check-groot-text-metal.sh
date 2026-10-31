#!/usr/bin/env bash
# Firm reason5: compile and run Coax graph with effects-only Metal adapter.
set -euo pipefail
cd "$(dirname "$0")/../../.."
weights="${1:?usage: check-groot-text-metal.sh WEIGHTS CAPSULE NEW_OUTPUT [VISION_OUTPUT]}"
capsule="${2:?}";work="${3:?}";coax="${COAX:-$PWD/coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}";python="${REFERENCE_PYTHON:-python3}"
[[ ! -e "$work" ]] || { echo 'output already exists' >&2;exit 2; };mkdir -p "$work"
vision=();if [[ $# -ge 4 ]];then vision=(--vision "$4");fi
"$python" psionic/tests/robotics/groot_text_metal_reference.py "$capsule" "$work/inputs" --prepare "${vision[@]}"
"$coax" metal-compute psionic/lib/device/tile.coax --release > "$work/tile.metal"
"$coax" metal-compute psionic/lib/robotics/groot-device-kernels.coax --release > "$work/device.metal"
"$coax" metal-compute psionic/lib/robotics/groot-text-kernels.coax --release > "$work/text.metal"
"$coax" metal-compute psionic/lib/robotics/groot-action-wide-kernels.coax --release > "$work/wide.metal"
"$coax" c psionic/lib/robotics/groot-text-metal-plan.coax --release --library --header "$work/text-plan.h" > "$work/text-plan.c"
"$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -ffp-contract=off -fno-fast-math -I psionic/platform -c psionic/platform/metal-host.m -o "$work/metal-host.o"
"$cc" -std=c11 -O2 -fblocks -mmacosx-version-min=15.0 -ffp-contract=off -fno-fast-math -I "$work" -I psionic/platform -I psionic/tests/robotics "$work/text-plan.c" psionic/tests/robotics/groot_text_metal.c "$work/metal-host.o" -framework Metal -framework Foundation -lm -o "$work/groot-text-metal"
"$work/groot-text-metal" "$weights" "$work/inputs" "$work/tile.metal" "$work/device.metal" "$work/text.metal" "$work" "$work/device.txt" "$work/wide.metal"
"$python" psionic/tests/robotics/groot_text_metal_reference.py "$capsule" "$work"
