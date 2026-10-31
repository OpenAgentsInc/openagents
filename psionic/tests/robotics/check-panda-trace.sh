#!/usr/bin/env bash
# Firm reason5: native compiler wrapper; C/stdlib SHA oracle are reason2.
set -euo pipefail
work="${1:?usage: check-panda-trace.sh OUTPUT_DIRECTORY}";mkdir -p "$work"
coax="${COAX:-coax/bin/coax}";compiler="${CC:-cc}";python="${PYTHON:-python3}"
for mode in debug release sanitized;do
  options=();flags=(-O0 -g)
  if [[ "$mode" == release ]];then options=(--release);flags=(-O2);fi
  if [[ "$mode" == sanitized ]];then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
  "$coax" c psionic/lib/robotics/panda-trace.coax --library "${options[@]}" --header "$work/panda-trace.h" > "$work/$mode.c"
  "$compiler" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" "$work/$mode.c" psionic/tests/robotics/panda_trace_native.c -lm -o "$work/$mode"
  "$work/$mode" "$work/$mode"
  "$python" psionic/tests/robotics/panda_trace_reference.py "$work/$mode" --record "$work/$mode.json"
done
for mode in release sanitized;do for suffix in inputs trace chains;do cmp "$work/debug.$suffix" "$work/$mode.$suffix";done;done
