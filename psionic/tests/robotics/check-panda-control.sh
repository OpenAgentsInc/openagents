#!/usr/bin/env bash
# Thin numerical/original-source telemetry oracle. Existing NumPy only.
set -euo pipefail
cd "$(dirname "$0")/../../.."
fixture="${1:?usage: check-panda-control.sh SOURCE_PCSAMPLE_DIRECTORY NEW_OUTPUT_DIRECTORY}"
work="${2:?new output directory required}"
coax="${COAX:-$PWD/coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}";python="${PANDA_REFERENCE_PYTHON:-$HOME/.cache/psionic/groot-reference-env/bin/python}"
"$python" -I psionic/tests/robotics/panda_control_reference.py prepare "$fixture" "$work"
for mode in debug release sanitized;do
  options=();flags=(-O0 -g)
  if [[ "$mode" == release ]];then options=(--release);flags=(-O2);fi
  if [[ "$mode" == sanitized ]];then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
  "$coax" c psionic/lib/robotics/panda-control.coax --library "${options[@]}" --header "$work/panda-control.h" > "$work/$mode.c"
  "$cc" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" "$work/$mode.c" psionic/tests/robotics/panda-control-native.c -lm -o "$work/$mode"
  "$work/$mode" "$work/controller-input.bin" "$work/$mode.controller.bin" "$work/matrix-input.bin" "$work/$mode.matrix.bin"
done
"$python" -I psionic/tests/robotics/panda_control_reference.py compare "$fixture" "$work"
