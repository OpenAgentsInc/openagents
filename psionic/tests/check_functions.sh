#!/usr/bin/env bash
# Focused graph-function authority, native/reference and book gate.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};jobs=4;mutate=0;list=0
units=(function-layout function-linearize function-budget function-transforms function-products function-batch);chosen=()
while [[ $# -gt 0 ]];do
 case "$1" in
  --jobs) [[ $# -ge 2 && $2 =~ ^[1-9][0-9]*$ ]] || exit 2;jobs=$2;shift 2;;
  --mutate) mutate=1;shift;; --list) list=1;shift;;
  *) item=${1%.coax};item=${item#psionic/lib/}
   case "$item" in functions|functions/all) chosen=("${units[@]}");;function-layout|function-linearize|function-budget|function-transforms|function-products|function-batch) chosen+=("$item");;*) echo "unknown function unit: $1" >&2;exit 2;;esac;shift;;
 esac
done
if [[ ${#chosen[@]} -eq 0 ]];then chosen=("${units[@]}");fi
if [[ $list == 1 ]];then printf '%s\n' "${chosen[@]}";exit;fi
work=$(mktemp -d "${TMPDIR:-/tmp}/psionic-functions.XXXXXX");trap 'rm -rf "$work"' EXIT
for unit in "${chosen[@]}";do
 source=psionic/lib/$unit.coax
 "$coax" test "$source"
 "$coax" test "$source" --release
 if [[ $mutate == 1 ]];then "$coax" mutate "$source" --root-only --jobs "$jobs";fi
done
COAX="$coax" CC="${COAX_CC:-${CC:-cc}}" psionic/tests/check-function-native.sh "$work/native"
for mode in debug release;do
 options=();if [[ $mode == release ]];then options=(--release);fi
 "$coax" run psionic/examples/function-transforms.coax ${options[@]+"${options[@]}"} > "$work/$mode.out"
 cmp psionic/fixtures/function-transforms.out "$work/$mode.out"
done
python3 psionic/tests/book.py
