#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
compiler=${COAX_TEST_COMPILER:-$PWD/coax/bin/coax}
ground_ray_out=$(mktemp -d "${TMPDIR:-/tmp}/aethyr-ground-ray.XXXXXX")
trap 'rm -rf "$ground_ray_out"' EXIT
for mode in debug release sanitized; do
  coax_flags=(); cc_flags=(-O0 -g)
  if [[ $mode == release ]]; then coax_flags=(--release); cc_flags=(-O2); fi
  if [[ $mode == sanitized ]]; then cc_flags=(-O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer); fi
  "$compiler" c aethyr/tests/ground-ray-native.coax --library "${coax_flags[@]}" \
    --header "$ground_ray_out/ground-ray-native.h" > "$ground_ray_out/ground-ray-native.c"
  "${CC:-cc}" -std=c11 -ffp-contract=off -fno-fast-math "${cc_flags[@]}" \
    -I "$ground_ray_out" "$ground_ray_out/ground-ray-native.c" aethyr/tests/ground-ray-native.c \
    -lm -o "$ground_ray_out/ground-ray-native"
  ASAN_OPTIONS=halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$ground_ray_out/ground-ray-native"
done
