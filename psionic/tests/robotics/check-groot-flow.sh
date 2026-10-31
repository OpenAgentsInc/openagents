#!/usr/bin/env bash
# Firm reason: 5 (thin compiler wrapper); C is independent numerical oracle (2).
set -euo pipefail
work="${1:?usage: check-groot-flow.sh OUTPUT_DIRECTORY}"
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
"$coax" c psionic/lib/robotics/groot-flow.coax --library --header "$work/groot-flow.h" > "$work/groot-flow-debug.c"
"${CC:-cc}" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/groot-flow-debug.c" psionic/tests/robotics/groot-flow-native.c -lm -o "$work/debug"
"$work/debug" "$work/time-debug.f32"
"$coax" c psionic/lib/robotics/groot-flow.coax --library --release --header "$work/groot-flow.h" > "$work/groot-flow-release.c"
"${CC:-cc}" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/groot-flow-release.c" psionic/tests/robotics/groot-flow-native.c -lm -o "$work/release"
"$work/release" "$work/time-release.f32"
"${CC:-cc}" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$work" "$work/groot-flow-debug.c" psionic/tests/robotics/groot-flow-native.c -lm -o "$work/sanitized"
"$work/sanitized" "$work/time-sanitized.f32"
