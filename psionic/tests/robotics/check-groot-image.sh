#!/usr/bin/env bash
# Firm reason5: compiler wrapper; independent C/OpenCV pixel oracle is reason2.
set -euo pipefail
work="${1:?usage: check-groot-image.sh OUTPUT_DIRECTORY}"
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
"$coax" c psionic/lib/robotics/groot-image.coax --library --header "$work/groot-image.h" > "$work/debug.c"
"${CC:-cc}" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/debug.c" psionic/tests/robotics/groot-image-native.c -lm -o "$work/debug"
"$work/debug"
"$coax" c psionic/lib/robotics/groot-image.coax --library --release --header "$work/groot-image.h" > "$work/release.c"
"${CC:-cc}" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/release.c" psionic/tests/robotics/groot-image-native.c -lm -o "$work/release"
"$work/release"
"${CC:-cc}" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$work" "$work/debug.c" psionic/tests/robotics/groot-image-native.c -lm -o "$work/sanitized"
"$work/sanitized"
if [[ -n "${PSIONIC_OPENCV_PYTHON:-}" ]]; then
  shared=(-shared -fPIC)
  if [[ "$(uname -s)" == Darwin ]]; then shared=(-dynamiclib); fi
  "${CC:-cc}" -std=c11 -O0 -ffp-contract=off -fno-fast-math "${shared[@]}" "$work/debug.c" -lm -o "$work/image.so"
  "$PSIONIC_OPENCV_PYTHON" psionic/tests/robotics/groot_image_reference.py --library "$work/image.so"
fi
