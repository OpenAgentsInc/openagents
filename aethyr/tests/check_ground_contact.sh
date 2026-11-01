#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
compiler=${COAX_TEST_COMPILER:-$PWD/coax/bin/coax}
ground_contact_out=$(mktemp -d "${TMPDIR:-/tmp}/aethyr-ground-contact.XXXXXX")
trap 'rm -rf "$ground_contact_out"' EXIT
for mode in debug release sanitized; do
  coax_flags=(); cc_flags=(-O0 -g)
  if [[ $mode == release ]]; then coax_flags=(--release); cc_flags=(-O2); fi
  if [[ $mode == sanitized ]]; then cc_flags=(-O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer); fi
  "$compiler" c aethyr/tests/ground-contact-native.coax --library "${coax_flags[@]}" \
    --header "$ground_contact_out/ground-contact-native.h" > "$ground_contact_out/ground-contact-native.c"
  "${CC:-cc}" -std=c11 -ffp-contract=off -fno-fast-math "${cc_flags[@]}" \
    -I "$ground_contact_out" "$ground_contact_out/ground-contact-native.c" aethyr/tests/ground-contact-native.c \
    -lm -o "$ground_contact_out/ground-contact-native"
  ASAN_OPTIONS=halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$ground_contact_out/ground-contact-native"
done
