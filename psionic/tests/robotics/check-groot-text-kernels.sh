#!/usr/bin/env bash
# Firm reason5: compiler wrapper; independent ABI/device/Torch oracle is reason2.
set -euo pipefail
work="${1:?usage: check-groot-text-kernels.sh NEW_OUTPUT_DIRECTORY}"
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
python="${GROOT_REFERENCE_PYTHON:-$HOME/.cache/psionic/groot-reference-env/bin/python}"
"$coax" metal-compute psionic/lib/robotics/groot-text-kernels.coax --release > "$work/kernels.metal"
"${CC:-cc}" -fobjc-arc -mmacosx-version-min=15.0 -O2 -c psionic/platform/metal-host.m -o "$work/metal-host.o"
for mode in debug release sanitized; do
  options=();flags=(-O0 -g)
  if [[ "$mode" == release ]]; then options=(--release);flags=(-O2); fi
  if [[ "$mode" == sanitized ]]; then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
  "$coax" c psionic/lib/robotics/groot-text-kernels.coax --library "${options[@]}" --header "$work/kernels.h" > "$work/$mode.c"
  "${CC:-cc}" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" -Ipsionic/platform "$work/$mode.c" psionic/tests/robotics/groot-text-kernels-native.c "$work/metal-host.o" -framework Metal -framework Foundation -lm -o "$work/$mode"
  "$work/$mode" "$work" "$mode" "$work/kernels.metal"
done
"$python" psionic/tests/robotics/groot_text_kernels_reference.py "$work"
