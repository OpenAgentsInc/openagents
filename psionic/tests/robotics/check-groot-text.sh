#!/usr/bin/env bash
# Firm reason5: thin compiler wrapper; C/Python are independent reason2 oracles.
set -euo pipefail
work="${1:?usage: check-groot-text.sh OUTPUT_DIRECTORY}";mkdir -p "$work"
coax="${COAX:-coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}"
"$coax" c psionic/lib/robotics/groot-text.coax --library --header "$work/groot-text.h" > "$work/debug.c"
"$coax" c psionic/lib/robotics/groot-text.coax --library --release --header "$work/groot-text.h" > "$work/release.c"
"$cc" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/debug.c" psionic/tests/robotics/groot-text-native.c -lm -o "$work/debug"
"$work/debug" "$work/debug.f32"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/release.c" psionic/tests/robotics/groot-text-native.c -lm -o "$work/release"
"$work/release" "$work/release.f32"
"$cc" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$work" "$work/debug.c" psionic/tests/robotics/groot-text-native.c -lm -o "$work/sanitized"
"$work/sanitized" "$work/sanitized.f32"
if [[ -n "${PSIONIC_GROOT_REFERENCE_PYTHON:-}" ]]; then
  "$PSIONIC_GROOT_REFERENCE_PYTHON" psionic/tests/robotics/groot_text_reference.py "$work/debug.f32" "$work/release.f32" "$work/sanitized.f32" --record "$work/reference.json"
fi
