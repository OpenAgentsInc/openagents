#!/usr/bin/env bash
# Firm reason: 5 (thin compiler wrapper); C and optional NumPy are oracles (2).
set -euo pipefail
work="${1:?usage: check-groot-embodiment.sh OUTPUT_DIRECTORY}"
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
"$coax" c psionic/lib/robotics/groot-embodiment.coax --library --header "$work/groot-embodiment.h" > "$work/debug.c"
"${CC:-cc}" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/debug.c" psionic/tests/robotics/groot-embodiment-native.c -lm -o "$work/debug"
"$work/debug"
"$coax" c psionic/lib/robotics/groot-embodiment.coax --library --release --header "$work/groot-embodiment.h" > "$work/release.c"
"${CC:-cc}" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/release.c" psionic/tests/robotics/groot-embodiment-native.c -lm -o "$work/release"
"$work/release"
"${CC:-cc}" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$work" "$work/debug.c" psionic/tests/robotics/groot-embodiment-native.c -lm -o "$work/sanitized"
"$work/sanitized"
if [[ -n "${PSIONIC_NUMPY_PYTHON:-}" ]]; then
  shared=(-shared -fPIC)
  if [[ "$(uname -s)" == Darwin ]]; then shared=(-dynamiclib); fi
  "${CC:-cc}" -std=c11 -O0 -ffp-contract=off -fno-fast-math "${shared[@]}" "$work/debug.c" -lm -o "$work/embodiment.so"
  "$PSIONIC_NUMPY_PYTHON" psionic/tests/robotics/groot_embodiment_reference.py "$work/embodiment.so"
fi
