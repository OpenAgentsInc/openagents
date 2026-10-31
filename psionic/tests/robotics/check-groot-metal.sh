#!/usr/bin/env bash
# Thin required-device original-weight harness; no CPU projection fallback.
set -euo pipefail
cd "$(dirname "$0")/../../.."
weights="${1:?usage: check-groot-metal.sh WEIGHTS_DIRECTORY NEW_OUTPUT_DIRECTORY}"
work="${2:?usage: check-groot-metal.sh WEIGHTS_DIRECTORY NEW_OUTPUT_DIRECTORY}"
if [[ -e "$work" ]];then echo "Metal output directory already exists: $work" >&2;exit 2;fi
mkdir -p "$work"
coax="${COAX:-$PWD/coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}";python="${REFERENCE_PYTHON:-python3}"
"$python" -I psionic/tests/robotics/groot_metal_reference.py "$weights" --authenticate-only
"$coax" metal-compute psionic/lib/device/tile.coax --release > "$work/tile.metal"
"$coax" metal-compute psionic/lib/robotics/groot-metal-kernels.coax --release > "$work/bias.metal"
"$coax" c psionic/lib/robotics/groot-metal-plan.coax --release --library --header "$work/plan.h" > "$work/plan.c"
"$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -ffp-contract=off -fno-fast-math -I psionic/platform -c psionic/platform/metal-host.m -o "$work/metal-host.o"
"$cc" -std=c11 -O2 -fblocks -mmacosx-version-min=15.0 -ffp-contract=off -fno-fast-math -I "$work" -I psionic/platform "$work/plan.c" psionic/tests/robotics/groot_metal.c "$work/metal-host.o" -framework Metal -framework Foundation -lm -o "$work/groot-metal"
"$work/groot-metal" "$weights" "$work/tile.metal" "$work/bias.metal" "$work/output.f32" "$work/device.txt"
"$python" -I psionic/tests/robotics/groot_metal_reference.py "$weights" --output "$work/output.f32" --device "$work/device.txt" --record "$work/record.json"
