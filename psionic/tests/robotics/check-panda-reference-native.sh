#!/usr/bin/env bash
# Firm reason2/5: changed reference FFI boundary, no model or GPU execution.
set -euo pipefail
pst="${1:?usage: check-panda-reference-native.sh ORIGINAL_PST BUILD [QUALIFIED_GENERATED_C_CACHE]}"
work="${2:?build directory required}"
cache="${3:-}"
mkdir -p "$work"
if [[ -n "$cache" ]];then
  cp "$cache/panda-policy-generated.c" "$cache/panda-policy-generated.h" "$work/"
else
  "${COAX:-coax/bin/coax}" c psionic/lib/robotics/panda-policy.coax --library --release --header "$work/panda-policy-generated.h" > "$work/panda-policy-generated.c"
fi
for mode in release sanitized;do
  flags=(-O2)
  if [[ "$mode" == sanitized ]];then flags=(-O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "${CC:-cc}" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" -Ipsionic/platform -Ipsionic/tests/robotics "$work/panda-policy-generated.c" psionic/tests/robotics/panda_reference_bridge.c psionic/tests/robotics/panda_reference_native.c -lm -o "$work/$mode"
  "$work/$mode" "$pst"
done
