#!/usr/bin/env bash
# Firm reason:5 (thin compiler wrapper); C is an independent oracle (2).
set -euo pipefail
work="${1:?usage: check-groot-vision.sh OUTPUT_DIRECTORY}"
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}"
"$coax" c psionic/lib/robotics/groot-vision.coax --library --header "$work/groot-vision.h" > "$work/vision-debug.c"
"$cc" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/vision-debug.c" psionic/tests/robotics/groot-vision-native.c -lm -o "$work/debug"
"$work/debug"
"$coax" c psionic/lib/robotics/groot-vision.coax --library --release --header "$work/groot-vision.h" > "$work/vision-release.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/vision-release.c" psionic/tests/robotics/groot-vision-native.c -lm -o "$work/release"
"$work/release"
"$cc" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$work" "$work/vision-debug.c" psionic/tests/robotics/groot-vision-native.c -lm -o "$work/sanitized"
"$work/sanitized"
