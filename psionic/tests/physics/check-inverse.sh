#!/usr/bin/env bash
# Firm reason5: build wrapper. C and stdlib analytic reference are reason2.
set -euo pipefail
work="${1:?usage: check-inverse.sh OUTPUT_DIRECTORY}";mkdir -p "$work"
coax="${COAX:-coax/bin/coax}";compiler="${CC:-cc}";python="${PYTHON:-python3}"
for mode in debug release sanitized;do
  options=();flags=(-O0 -g)
  if [[ "$mode" == release ]];then options=(--release);flags=(-O2);fi
  if [[ "$mode" == sanitized ]];then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
  "$coax" c psionic/lib/physics/inverse-evaluate.coax --library "${options[@]}" --header "$work/inverse.h" > "$work/$mode.c"
  "$compiler" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" "$work/$mode.c" psionic/tests/physics/inverse_native.c -lm -o "$work/$mode"
  "$work/$mode" "$work/$mode"
  "$python" psionic/tests/physics/inverse_reference.py "$work/$mode" --cache "$work/oracle.json"
  "$python" psionic/tests/physics/inverse_coverage.py "$work/$mode"
done
for mode in release sanitized;do
  for suffix in data.f64 states.f64 gradients.json fits.json;do cmp "$work/debug.$suffix" "$work/$mode.$suffix";done
done
