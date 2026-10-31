#!/usr/bin/env bash
# Firm reasons: 5 (thin build wrapper), 2 (independent oracle tooling).
set -euo pipefail
work="${1:?usage: check-groot-device.sh NEW_OUTPUT_DIRECTORY}"
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
compiler="${CC:-cc}"
python="${GROOT_REFERENCE_PYTHON:-$HOME/.cache/psionic/groot-reference-env/bin/python}"
"$coax" metal-compute psionic/lib/robotics/groot-device-kernels.coax --release > "$work/groot-device.metal"
"$compiler" -fobjc-arc -mmacosx-version-min=15.0 -O2 -c psionic/platform/metal-host.m -o "$work/metal-host.o"
for mode in debug release sanitized; do
  options=()
  flags=(-O0 -g)
  if [[ "$mode" == release ]]; then options=(--release); flags=(-O2); fi
  if [[ "$mode" == sanitized ]]; then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
  "$coax" c psionic/lib/robotics/groot-device-kernels.coax --library "${options[@]}" --header "$work/groot-device.h" > "$work/$mode.c"
  "$compiler" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" -Ipsionic/platform "$work/$mode.c" psionic/tests/robotics/groot-device-native.c "$work/metal-host.o" -framework Metal -framework Foundation -lm -o "$work/$mode"
  "$work/$mode" "$work" "$mode" "$work/groot-device.metal"
done
"$python" psionic/tests/robotics/groot_device_reference.py "$work"
