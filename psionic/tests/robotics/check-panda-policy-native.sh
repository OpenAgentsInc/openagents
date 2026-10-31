#!/usr/bin/env bash
# Firm reason5: thin native compiler/effect wrapper.
set -euo pipefail
work="${1:-/tmp/panda-policy-native}"
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
cc="${CC:-cc}"
"$coax" c psionic/lib/robotics/panda-policy.coax --library --release --header "$work/panda-policy-generated.h" > "$work/panda-policy-generated.c"
for mode in debug release sanitized; do
  flags=(-O0 -g)
  if [[ "$mode" == release ]]; then flags=(-O2); fi
  if [[ "$mode" == sanitized ]]; then flags=(-O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer); fi
  "$cc" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" "$work/panda-policy-generated.c" psionic/tests/robotics/panda-policy-native.c -lm -o "$work/$mode"
  "$work/$mode"
done
