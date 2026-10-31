#!/usr/bin/env bash
# Firm reasons5(thin wrapper),2(independent metadata/native ABI oracle).
set -euo pipefail
work="${1:?usage: check-groot-action-dynamic-native.sh OUTPUT_DIRECTORY}"
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
compiler="${CC:-cc}"
for mode in debug release sanitized; do
  options=()
  flags=(-O0 -g)
  if [[ "$mode" == release ]]; then options=(--release); flags=(-O2); fi
  if [[ "$mode" == sanitized ]]; then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
  "$coax" c psionic/lib/robotics/groot-action-dynamic-plan.coax --library "${options[@]}" --header "$work/action-dynamic-plan.h" > "$work/$mode.c"
  "$compiler" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" "$work/$mode.c" psionic/tests/robotics/groot-action-dynamic-native.c -lm -o "$work/$mode"
  "$work/$mode"
done
