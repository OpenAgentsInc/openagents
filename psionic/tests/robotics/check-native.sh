#!/usr/bin/env bash
# Firm reason: 5 (thin compiler wrapper); native.c is an independent oracle (2).
set -euo pipefail
work="${1:?usage: check-native.sh OUTPUT_DIRECTORY}"
mkdir -p "$work"
"${COAX:-coax/bin/coax}" c psionic/lib/robotics/learn.coax --library --header "$work/robotics.h" > "$work/robotics-debug.c"
"${CC:-cc}" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/robotics-debug.c" psionic/tests/robotics/native.c -lm -o "$work/debug"
"$work/debug"
"${COAX:-coax/bin/coax}" c psionic/lib/robotics/learn.coax --library --release --header "$work/robotics.h" > "$work/robotics-release.c"
"${CC:-cc}" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/robotics-release.c" psionic/tests/robotics/native.c -lm -o "$work/release"
"$work/release"
"${CC:-cc}" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$work" "$work/robotics-debug.c" psionic/tests/robotics/native.c -lm -o "$work/sanitized"
"$work/sanitized"
