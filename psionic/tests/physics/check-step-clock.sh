#!/usr/bin/env bash
# Firm reason2/5: native/compiler and independent arithmetic verification tooling.
set -euo pipefail
work="${1:?usage: check-step-clock.sh NEW_DIRECTORY [PRIOR_QUALIFIED_DIRECTORY]}"
qualified="${2:-}"
[[ ! -e "$work" ]] || { echo 'new output directory required' >&2;exit 2; }
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}";cc="${CC:-cc}";python="${PYTHON:-python3}"
"$coax" c psionic/lib/physics/oscillator-clock.coax --library --header "$work/oscillator-clock.h" > "$work/debug.c"
"$coax" c psionic/lib/physics/oscillator-clock.coax --library --release --header "$work/release.h" > "$work/release.c"
cmp "$work/oscillator-clock.h" "$work/release.h"
for mode in debug release sanitized;do
  source="$work/debug.c";flags=(-O0 -g)
  if [[ "$mode" == release ]];then source="$work/release.c";flags=(-O2);fi
  if [[ "$mode" == sanitized ]];then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" "$source" psionic/tests/physics/step-clock-native.c -lm -o "$work/$mode"
  "$work/$mode" "$work/$mode"
done
for mode in release sanitized;do
  cmp "$work/debug.events.i64" "$work/$mode.events.i64"
  cmp "$work/debug.summary.json" "$work/$mode.summary.json"
done
if [[ -n "$qualified" ]];then
  "$python" psionic/tests/physics/step_clock_reuse.py "$work" "$qualified"
else
  "$python" psionic/tests/physics/step_clock_reference.py "$work/debug" --record "$work/reference.json"
  for fixture in psionic/fixtures/physics/clock-compile-fail/*.coax;do
    name="$(basename "$fixture" .coax)"
    if "$coax" check "$fixture" > "$work/$name.diagnostic" 2>&1;then echo "unexpected compile success: $fixture" >&2;exit 1;fi
    rg -q 'error\[E0317\]: argument' "$work/$name.diagnostic"
  done
  echo 'Nominal clock-domain compile-fail fixtures refused.'
fi
