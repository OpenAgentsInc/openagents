#!/usr/bin/env bash
# Firm reasons5(thin wrapper),2(independent native metadata/ABI oracle).
set -euo pipefail
work="${1:?usage: check-groot-action-plan-native.sh OUTPUT_DIRECTORY [EXECUTED_GPU_BUILD_DIRECTORY [EXECUTED_TRANSCRIPT]]}"
executed="${2:-}"
if [[ -n "${3:-}" && -z "$executed" ]];then echo 'an executed transcript requires its original GPU build directory' >&2;exit 2;fi
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
compiler="${COAX_CC:-${CC:-cc}}"
if [[ -n "$executed" ]]; then
if [[ -n "${3:-}" ]]; then
  cp "$3" "$work/executed.transcript"
else
  "$compiler" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$executed" "$executed/action-plan.c" psionic/tests/robotics/groot-action-plan-native.c -lm -o "$work/executed"
  "$work/executed" "$work/executed.transcript"
fi
fi
for mode in debug release sanitized; do
  options=()
  flags=(-O0 -g)
  if [[ "$mode" == release ]]; then options=(--release); flags=(-O2); fi
  if [[ "$mode" == sanitized ]]; then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer); fi
  "$coax" c psionic/lib/robotics/groot-action-metal-plan.coax --library "${options[@]}" --header "$work/action-plan.h" > "$work/$mode.c"
  "$compiler" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" "$work/$mode.c" psionic/tests/robotics/groot-action-plan-native.c -lm -o "$work/$mode"
  "$work/$mode" "$work/$mode.transcript"
  if [[ -n "$executed" ]];then
    cmp "$work/executed.transcript" "$work/$mode.transcript"
  elif [[ "$mode" != debug ]];then
    cmp "$work/debug.transcript" "$work/$mode.transcript"
  fi
done

if [[ -n "$executed" ]];then
  echo 'action plan: three native modes match the prior executed GPU graph transcript'
else
  echo 'action plan: three portable native modes agree on the complete descriptor transcript; no GPU execution claimed'
fi
