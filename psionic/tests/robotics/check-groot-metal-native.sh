#!/usr/bin/env bash
# Thin effects wrapper: independent native oracle for new Coax GPU helpers.
set -euo pipefail
cd "$(dirname "$0")/../../.."
coax="${COAX:-$PWD/coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}"
work="$(mktemp -d "${TMPDIR:-/tmp}/psionic-groot-metal-native.XXXXXX")"
trap 'rm -rf "$work"' EXIT
for root in plan kernels;do
  header=plan;if [[ "$root" == kernels ]];then header=bias;fi
  "$coax" c "psionic/lib/robotics/groot-metal-$root.coax" --library --header "$work/$header.h" > "$work/$root-debug.c"
  "$coax" c "psionic/lib/robotics/groot-metal-$root.coax" --release --library --header "$work/$header.h" > "$work/$root-release.c"
done
"$cc" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/plan-debug.c" "$work/kernels-debug.c" psionic/tests/robotics/groot_metal_native.c -lm -o "$work/debug"
"$work/debug"
"$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -I"$work" "$work/plan-release.c" "$work/kernels-release.c" psionic/tests/robotics/groot_metal_native.c -lm -o "$work/release"
"$work/release"
"$cc" -std=c11 -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -ffp-contract=off -fno-fast-math -I"$work" "$work/plan-debug.c" "$work/kernels-debug.c" psionic/tests/robotics/groot_metal_native.c -lm -o "$work/sanitized"
"$work/sanitized"
