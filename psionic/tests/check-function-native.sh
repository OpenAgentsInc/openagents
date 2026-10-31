#!/usr/bin/env bash
# Firm reason2/5: small-graph native/compiler and independent numeric tooling.
set -euo pipefail
work="${1:?usage: check-function-native.sh NEW_BUILD [GENERATED_C_CACHE] [PRIOR_REFERENCE_RESULTS]}"
cache="${2:-}"
golden="${3:-}"
[[ ! -e "$work" ]] || { echo 'new native output directory required' >&2;exit 2; }
mkdir -p "$work"
cc="${CC:-cc}"
if [[ -n "$cache" ]];then
  cp "$cache/function-batch.c" "$cache/function-batch.h" "$work/"
else
  "${COAX:-coax/bin/coax}" c psionic/lib/function-batch.coax --library --release --header "$work/function-batch.h" > "$work/function-batch.c"
fi
for mode in debug release sanitized;do
  flags=(-O0 -g)
  if [[ "$mode" == release ]];then flags=(-O2);fi
  if [[ "$mode" == sanitized ]];then flags=(-O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer);fi
  "$cc" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" "$work/function-batch.c" psionic/tests/function-native.c -lm -o "$work/$mode"
  "$work/$mode" "$work/$mode-results.jsonl"
done
cmp "$work/debug-results.jsonl" "$work/release-results.jsonl"
cmp "$work/sanitized-results.jsonl" "$work/release-results.jsonl"
if [[ -n "$golden" ]];then
  cmp "$golden" "$work/release-results.jsonl"
  echo 'Current native outputs bit-identical to previously qualified independent reference results; independent oracle reused.'
else
  python="${PSIONIC_NUMPY_PYTHON:-python3}"
  if [[ -z "${PSIONIC_NUMPY_PYTHON:-}" ]] && ! "$python" -c 'import numpy' >/dev/null 2>&1 && [[ -x "$HOME/.cache/psionic/groot-reference-env/bin/python" ]];then
    python="$HOME/.cache/psionic/groot-reference-env/bin/python"
  fi
  "$python" psionic/tests/function_reference.py "$work/release-results.jsonl" --record "$work/reference.json"
fi
