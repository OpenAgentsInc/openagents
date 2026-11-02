#!/usr/bin/env bash
# Reuse already qualified generated wrappers; qualify only the new real host slice.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
output=${1:?usage: systemone-process-lifecycle.sh OUTPUT_DIR GENERATED_WRAPPERS_DIR}
generated=${2:?usage: systemone-process-lifecycle.sh OUTPUT_DIR GENERATED_WRAPPERS_DIR}
if [[ $output != /* ]]; then output="$PWD/$output"; fi
generated=$(cd "$generated" && pwd -P)
if [[ -e $output || -L $output ]]; then echo "refusing existing evidence: $output" >&2; exit 2; fi
for mode in debug release; do
  [[ -f $generated/$mode/systemone-lifecycle.c && -f $generated/$mode/systemone-lifecycle.h ]] || { echo "missing qualified generated wrappers" >&2; exit 2; }
done
mkdir -p "$(dirname "$output")"; mkdir -m 700 "$output"
cd "$root"
for mode in debug release; do
  work="$output/$mode";mkdir "$work";optimization=-O0
  if [[ $mode == release ]];then optimization=-O2;fi
  cp "$generated/$mode/systemone-lifecycle.c" "$generated/$mode/systemone-lifecycle.h" "$work/"
  for sanitize in plain sanitized;do
    sanitizer=();if [[ $sanitize == sanitized ]];then sanitizer=(-fsanitize=address,undefined -fno-sanitize-recover=all);fi
    "${COAX_CC:-cc}" -std=c11 "$optimization" -g "${sanitizer[@]}" coax/tests/systemone-fakecurl.c -o "$work/child-$sanitize"
    "${COAX_CC:-cc}" -std=c11 "$optimization" -g -ffp-contract=off "${sanitizer[@]}" -I "$work" -I coax/platform "$work/systemone-lifecycle.c" coax/platform/process-native.c coax/tests/systemone-process-lifecycle.c -lm -o "$work/$sanitize"
    "$work/$sanitize" "$work/child-$sanitize"
  done
done
