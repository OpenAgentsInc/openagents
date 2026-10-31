#!/usr/bin/env bash
# Firm reasons: 5 (thin build/effect wrapper), 2 (independent recorded oracle).
set -euo pipefail
weights="${1:?usage: check-groot-action-metal.sh FULL_SHARDS REFERENCE_CAPSULE NEW_OUTPUT_DIRECTORY}"
capsule="${2:?reference capsule required}"
work="${3:?new output directory required}"
mkdir -p "$work/output"
coax="${COAX:-coax/bin/coax}"
compiler="${CC:-cc}"
python="${GROOT_REFERENCE_PYTHON:-$HOME/.cache/psionic/groot-reference-env/bin/python}"
backbone_options=()
if [[ -n "${4:-}" ]]; then backbone_options=(--backbone "$4"); fi
"$python" psionic/tests/robotics/groot_action_reference.py "$capsule" "$work" --prepare --weights "$weights" "${backbone_options[@]}"
"$coax" c psionic/lib/robotics/groot-action-metal-plan.coax --library --release --header "$work/action-plan.h" > "$work/action-plan.c"
"$coax" metal-compute psionic/lib/device/tile.coax --release > "$work/tile.metal"
"$coax" metal-compute psionic/lib/robotics/groot-device-kernels.coax --release > "$work/device.metal"
"$coax" metal-compute psionic/lib/robotics/groot-action-metal-kernels.coax --release > "$work/action.metal"
"$coax" metal-compute psionic/lib/robotics/groot-action-wide-kernels.coax --release > "$work/wide.metal"
"$compiler" -fobjc-arc -mmacosx-version-min=15.0 -O2 -c psionic/platform/metal-host.m -o "$work/metal-host.o"
"$compiler" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" -Ipsionic/platform -Ipsionic/tests/robotics "$work/action-plan.c" psionic/tests/robotics/groot_action_metal.c "$work/metal-host.o" -framework Metal -framework Foundation -lm -o "$work/action-metal"
"$work/action-metal" "$weights" "$work" "$work/tile.metal" "$work/device.metal" "$work/action.metal" "$work/output" "$work/device.txt"
"$python" psionic/tests/robotics/groot_action_reference.py "$capsule" "$work"
