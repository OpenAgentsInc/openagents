#!/usr/bin/env bash
# Native-only adapter verification; no datasets, training runs or old mutations.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-coax/bin/coax}
cc=${COAX_CC:-${CC:-cc}}
out=$(mktemp -d "${TMPDIR:-/tmp}/module-train-native.XXXXXXXX")
trap 'rm -rf "$out"' EXIT
for mode in debug release; do
  dir="$out/$mode"
  mkdir -p "$dir"
  flags=(); opt=-O0
  if [[ $mode == release ]]; then flags+=(--release); opt=-O2; fi
  "$coax" c psionic/tests/module-train-native.coax --library \
    --header "$dir/module-train-native.h" "${flags[@]+"${flags[@]}"}" > "$dir/module-train-native.c"
  for kind in plain sanitized; do
    sanitize=()
    if [[ $kind == sanitized ]]; then sanitize=(-fsanitize=address,undefined -fno-omit-frame-pointer); fi
    "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math \
      -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas \
      "${sanitize[@]+"${sanitize[@]}"}" -I"$dir" \
      "$dir/module-train-native.c" psionic/tests/module-train-native.c -lm -o "$dir/$kind"
    echo "module-train native $mode $kind:"
    ASAN_OPTIONS=detect_leaks=0:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 "$dir/$kind"
  done
done
