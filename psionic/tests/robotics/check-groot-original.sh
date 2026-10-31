#!/usr/bin/env bash
# Firm reason: 5 (thin compiler wrapper); Python is an independent oracle (2).
set -euo pipefail
cd "$(dirname "$0")/../../.."
weights="${1:?usage: check-groot-original.sh WEIGHTS_DIRECTORY NEW_OUTPUT_DIRECTORY}"
work="${2:?usage: check-groot-original.sh WEIGHTS_DIRECTORY NEW_OUTPUT_DIRECTORY}"
if [[ -e "$work" ]]; then echo "original state-encoder output directory already exists: $work" >&2;exit 2;fi
mkdir -p "$work"
coax="${COAX:-$PWD/coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}"
"$coax" c psionic/lib/robotics/groot-linear.coax --library --header "$work/groot-linear.h" > "$work/linear-debug.c"
"$cc" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/linear-debug.c" psionic/tests/robotics/groot-original-native.c -lm -o "$work/debug"
"$work/debug" "$weights" "$work/debug.bin"
"$coax" c psionic/lib/robotics/groot-linear.coax --library --release --header "$work/groot-linear.h" > "$work/linear-release.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/linear-release.c" psionic/tests/robotics/groot-original-native.c -lm -o "$work/release"
"$work/release" "$weights" "$work/release.bin"
"$cc" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$work" "$work/linear-debug.c" psionic/tests/robotics/groot-original-native.c -lm -o "$work/sanitized"
"$work/sanitized" "$weights" "$work/sanitized.bin"
"${REFERENCE_PYTHON:-python3}" -I psionic/tests/robotics/groot_original_reference.py "$weights" "$work/debug.bin" "$work/release.bin" "$work/sanitized.bin" --record "$work/record.json"
