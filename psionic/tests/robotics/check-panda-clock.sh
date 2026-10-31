#!/usr/bin/env bash
# Firm reason2/5: new paired ABI verification only, no SDK/model execution.
set -euo pipefail
work="${1:?usage: check-panda-clock.sh NEW_DIRECTORY}"
[[ ! -e "$work" ]] || { echo 'new output directory required' >&2;exit 2; };mkdir -p "$work"
coax="${COAX:-coax/bin/coax}";cc="${CC:-cc}"
"$coax" c psionic/lib/robotics/panda-clock-adapter.coax --library --header "$work/panda-clock.h" > "$work/debug.c"
"$coax" c psionic/lib/robotics/panda-clock-adapter.coax --library --release --header "$work/release.h" > "$work/release.c"
cmp "$work/panda-clock.h" "$work/release.h"
for mode in debug release sanitized;do
  source="$work/debug.c";flags=(-O0 -g)
  if [[ "$mode" == release ]];then source="$work/release.c";flags=(-O2);fi
  if [[ "$mode" == sanitized ]];then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" "$source" psionic/tests/robotics/panda-clock-native.c -lm -o "$work/$mode"
  "$work/$mode" > "$work/$mode.out"
done
cmp "$work/debug.out" "$work/release.out";cmp "$work/debug.out" "$work/sanitized.out"
cat "$work/debug.out"
