#!/usr/bin/env bash
# Firm reasons5(thin compiler/effect wrapper),2(independent reference tooling).
set -euo pipefail
weights="${1:?usage: check-groot-action-dynamic-metal.sh FULL_SHARDS CAPSULE NEW_OUTPUT [COAX_BACKBONE]}"
capsule="${2:?reference capsule required}"
work="${3:?new output directory required}"
mkdir -p "$work/output"
coax="${COAX:-coax/bin/coax}"
compiler="${CC:-cc}"
python="${GROOT_REFERENCE_PYTHON:-$HOME/.cache/psionic/groot-reference-env/bin/python}"
backbone_options=()
if [[ -n "${4:-}" ]]; then backbone_options=(--backbone "$4"); fi
"$python" psionic/tests/robotics/groot_action_dynamic_reference.py "$capsule" "$work" --prepare --weights "$weights" "${backbone_options[@]}"
tokens=$("$python" -c 'import json,sys; print(json.load(open(sys.argv[1]))["request_tokens"])' "$capsule/record.json")
"$coax" c psionic/lib/robotics/groot-action-dynamic-plan.coax --library --release --header "$work/action-dynamic-plan.h" > "$work/action-dynamic-plan.c"
if [[ -n "${GROOT_SHADER_CACHE:-}" ]]; then
  cp "$GROOT_SHADER_CACHE/tile.metal" "$work/tile.metal"
  cp "$GROOT_SHADER_CACHE/device.metal" "$work/device.metal"
  cp "$GROOT_SHADER_CACHE/action.metal" "$work/action.metal"
  cp "$GROOT_SHADER_CACHE/wide.metal" "$work/wide.metal"
else
  "$coax" metal-compute psionic/lib/device/tile.coax --release > "$work/tile.metal"
  "$coax" metal-compute psionic/lib/robotics/groot-device-kernels.coax --release > "$work/device.metal"
  "$coax" metal-compute psionic/lib/robotics/groot-action-metal-kernels.coax --release > "$work/action.metal"
  "$coax" metal-compute psionic/lib/robotics/groot-action-wide-kernels.coax --release > "$work/wide.metal"
fi
"$compiler" -fobjc-arc -mmacosx-version-min=15.0 -O2 -c psionic/platform/metal-host.m -o "$work/metal-host.o"
"$compiler" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" -Ipsionic/platform -Ipsionic/tests/robotics "$work/action-dynamic-plan.c" psionic/tests/robotics/groot_action_dynamic_metal.c "$work/metal-host.o" -framework Metal -framework Foundation -lm -o "$work/action-dynamic-metal"
"$work/action-dynamic-metal" "$weights" "$work" "$work/tile.metal" "$work/device.metal" "$work/action.metal" "$work/output" "$work/device.txt" "$tokens"
"$python" psionic/tests/robotics/groot_action_dynamic_reference.py "$capsule" "$work"
