#!/usr/bin/env bash
# Native RESIDUAL checks only; no data, learning, pure-group repeats or mutations.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-coax/bin/coax}
cc=${COAX_CC:-${CC:-cc}}
out=${1:-${RESIDUAL_NATIVE_OUT:-}}
temporary=0
if [[ -z $out ]]; then out=$(mktemp -d "${TMPDIR:-/tmp}/residual-native.XXXXXXXX");temporary=1;fi
mkdir -p "$out"
out=$(cd "$out" && pwd)
cleanup(){ if [[ $temporary == 1 ]];then rm -rf "$out";fi; }
trap cleanup EXIT
for mode in debug release;do
  dir="$out/$mode";mkdir -p "$dir"
  flags=();opt=-O0
  if [[ $mode == release ]];then flags+=(--release);opt=-O2;fi
  "$coax" c psionic/tests/residual-native.coax --library --run-steps 250000000 \
    --header "$dir/residual-native.h" "${flags[@]+"${flags[@]}"}" > "$dir/residual-native.c"
  for kind in plain sanitized;do
    checks=()
    if [[ $kind == sanitized ]];then checks=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
    "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
      -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas \
      "${checks[@]+"${checks[@]}"}" -I"$dir" "$dir/residual-native.c" psionic/tests/residual-native.c -lm -o "$dir/$kind"
    echo "RESIDUAL native $mode $kind:"
    ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$dir/$kind"
  done
done
