#!/usr/bin/env bash
# Independent scalar/native oracle for new Coax vision graph/device helpers.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}"
work="$(mktemp -d "${TMPDIR:-/tmp}/psionic-groot-vision-native.XXXXXX")"
trap 'rm -rf "$work"' EXIT
for source in metal-plan device;do
  name=plan;header=vision-plan;if [[ "$source" == device ]];then name=device;header=vision-device;fi
  "$coax" c "psionic/lib/robotics/groot-vision-$source.coax" --library --header "$work/$header.h" > "$work/$name-debug.c"
  "$coax" c "psionic/lib/robotics/groot-vision-$source.coax" --release --library --header "$work/$header.h" > "$work/$name-release.c"
done
"$cc" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/plan-debug.c" "$work/device-debug.c" psionic/tests/robotics/groot_vision_metal_native.c -lm -o "$work/debug"
"$work/debug"
"$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -I"$work" "$work/plan-release.c" "$work/device-release.c" psionic/tests/robotics/groot_vision_metal_native.c -lm -o "$work/release"
"$work/release"
"$cc" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -ffp-contract=off -fno-fast-math -I"$work" "$work/plan-debug.c" "$work/device-debug.c" psionic/tests/robotics/groot_vision_metal_native.c -lm -o "$work/sanitized"
"$work/sanitized"
