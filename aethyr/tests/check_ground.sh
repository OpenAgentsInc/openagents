#!/usr/bin/env bash
# Focused native ground gate. Coax owns the sampler; C supplies an independent plane oracle.
set -euo pipefail
cd "$(dirname "$0")/../.."
compiler=${COAX_TEST_COMPILER:-$PWD/coax/bin/coax}
ground_check_out=$(mktemp -d "${TMPDIR:-/tmp}/aethyr-ground.XXXXXX")
trap 'rm -rf "$ground_check_out"' EXIT
for mode in debug release sanitized; do
  coax_flags=()
  cc_flags=(-O0 -g)
  if [[ $mode == release ]]; then coax_flags=(--release); cc_flags=(-O2); fi
  if [[ $mode == sanitized ]]; then cc_flags=(-O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer); fi
  "$compiler" c aethyr/tests/ground-native.coax --library "${coax_flags[@]}" \
    --header "$ground_check_out/ground-native.h" > "$ground_check_out/ground-native.c"
  "${CC:-cc}" -std=c11 -ffp-contract=off -fno-fast-math "${cc_flags[@]}" \
    -I "$ground_check_out" "$ground_check_out/ground-native.c" aethyr/tests/ground-native.c \
    -lm -o "$ground_check_out/ground-native"
  ASAN_OPTIONS=halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$ground_check_out/ground-native"
done
for name in time raw; do
  status=0
  "$compiler" check "aethyr/tests/ground-refuse-$name.coax" --json > "$ground_check_out/refusal" 2>&1 || status=$?
  [[ $status == 1 ]] && rg -q '"code": "E0317"' "$ground_check_out/refusal"
  rg -Fq "ground-refuse-$name.coax" "$ground_check_out/refusal"
done
echo 'ground units: time and raw coordinates are refused with E0317'
