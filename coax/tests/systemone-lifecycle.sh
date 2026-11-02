#!/usr/bin/env bash
# Bounded offline generated-Coax lifecycle checks; no service or real process host.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
compiler=${COAX:-$root/coax/bin/coax}
output=${1:?usage: systemone-lifecycle.sh OUTPUT_DIR [debug|release|all]}
if [[ $output != /* ]]; then output="$PWD/$output"; fi
selection=${2-all}
case "$selection" in
  debug|release|all) ;;
  *) echo "mode must be debug, release or all" >&2; exit 2 ;;
esac
if [[ -e $output || -L $output ]]; then
  echo "refusing to replace existing lifecycle evidence: $output" >&2
  exit 2
fi
mkdir -p "$(dirname "$output")"
mkdir -m 700 "$output"
cd "$root"
for mode in debug release; do
  if [[ $selection != all && $selection != "$mode" ]]; then continue; fi
  work="$output/$mode"; mkdir -p "$work"
  flags=(); optimization=-O0
  if [[ $mode == release ]]; then flags+=(--release); optimization=-O2; fi
  "$compiler" c coax/tests/systemone-lifecycle.coax --library "${flags[@]}" --header "$work/systemone-lifecycle.h" > "$work/systemone-lifecycle.c"
  for sanitize in plain sanitized; do
    sanitizer=()
    if [[ $sanitize == sanitized ]]; then sanitizer=(-fsanitize=address,undefined -fno-sanitize-recover=all); fi
    "${COAX_CC:-cc}" -std=c11 "$optimization" -g -ffp-contract=off "${sanitizer[@]}" -I "$work" "$work/systemone-lifecycle.c" coax/tests/systemone-lifecycle.c -lm -o "$work/$sanitize"
    "$work/$sanitize"
  done
done
