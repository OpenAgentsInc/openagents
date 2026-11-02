#!/usr/bin/env bash
# Production legacy ABI checks, not another run of the already qualified pure sampler.
set -euo pipefail
cd "$(dirname "$0")/../.."
compiler=${COAX_TEST_COMPILER:-$PWD/coax/bin/coax}
terrain_check_out=$(mktemp -d "${TMPDIR:-/tmp}/aethyr-terrain.XXXXXX")
trap 'rm -rf "$terrain_check_out"' EXIT
for mode in debug release sanitized; do
  coax_flags=()
  cc_flags=(-O0 -g)
  if [[ $mode == release ]]; then coax_flags=(--release); cc_flags=(-O2); fi
  if [[ $mode == sanitized ]]; then cc_flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
  "$compiler" c aethyr/terrain.coax --library "${coax_flags[@]}" \
    --header "$terrain_check_out/terrain-native.h" > "$terrain_check_out/terrain-native.c"
  "${CC:-cc}" -std=c11 -ffp-contract=off -fno-fast-math "${cc_flags[@]}" \
    -I "$terrain_check_out" "$terrain_check_out/terrain-native.c" aethyr/tests/terrain-native.c \
    -lm -o "$terrain_check_out/terrain-native"
  ASAN_OPTIONS=halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$terrain_check_out/terrain-native"
done
