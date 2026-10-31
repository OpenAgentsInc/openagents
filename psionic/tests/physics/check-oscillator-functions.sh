#!/usr/bin/env bash
# Focused generated-C and independent polynomial qualification, no device/model.
set -euo pipefail
cd "$(dirname "$0")/../../.."
work=${1:?usage: check-oscillator-functions.sh NEW_OUTPUT_DIRECTORY}
[[ ! -e "$work" ]] || { echo 'new output directory required' >&2;exit 2; }
mkdir -p "$work";coax=${COAX:-$PWD/coax/bin/coax};cc=${CC:-cc}
"$coax" c psionic/lib/physics/oscillator-functions.coax --library --release --header "$work/oscillator-functions.h" > "$work/oscillator-functions.c"
for mode in debug release sanitized;do
 flags=(-O0 -g)
 if [[ $mode == release ]];then flags=(-O2);fi
 if [[ $mode == sanitized ]];then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
 "$cc" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work" "$work/oscillator-functions.c" psionic/tests/physics/oscillator_functions_native.c -lm -o "$work/$mode"
 "$work/$mode" "$work/$mode.jsonl"
 if [[ $mode == debug ]];then python3 psionic/tests/physics/oscillator_functions_reference.py "$work/$mode.jsonl" --output "$work/reference.json";else cmp "$work/debug.jsonl" "$work/$mode.jsonl";fi
done
