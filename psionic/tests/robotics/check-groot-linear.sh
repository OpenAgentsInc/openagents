#!/usr/bin/env bash
# Firm reason: 5 (thin compiler wrapper); C is an independent oracle (2).
set -euo pipefail
work="${1:?usage: check-groot-linear.sh OUTPUT_DIRECTORY}"
mkdir -p "$work"
"${COAX:-coax/bin/coax}" c psionic/lib/robotics/groot-linear.coax --library --header "$work/groot-linear.h" > "$work/linear-debug.c"
"${CC:-cc}" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/linear-debug.c" psionic/tests/robotics/groot-linear-native.c -lm -o "$work/debug"
"$work/debug"
"${COAX:-coax/bin/coax}" c psionic/lib/robotics/groot-linear.coax --library --release --header "$work/groot-linear.h" > "$work/linear-release.c"
"${CC:-cc}" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/linear-release.c" psionic/tests/robotics/groot-linear-native.c -lm -o "$work/release"
"$work/release"
"${CC:-cc}" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$work" "$work/linear-debug.c" psionic/tests/robotics/groot-linear-native.c -lm -o "$work/sanitized"
"$work/sanitized"
