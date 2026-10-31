#!/usr/bin/env bash
# Focused policy/native gate: no world rendering, crypto, GPU/model, or mutation closure.
set -euo pipefail
cd "$(dirname "$0")/.."
work=$(mktemp -d "${TMPDIR:-/tmp}/everglade-guard.XXXXXX")
trap 'rm -rf "$work"' EXIT
for mode in debug release sanitized; do
  emit=(); flags=(-O0 -g)
  if [[ "$mode" == release ]]; then emit=(--release);flags=(-O2);fi
  if [[ "$mode" == sanitized ]]; then flags=(-O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer);fi
  source_c="$work/guard-$mode.c"
  if [[ "$mode" == sanitized ]]; then source_c="$work/guard-debug.c";else
    coax/bin/coax c verse/everglade/guard-decision.coax --library --header "$work/Guard-decision.h" ${emit[@]+"${emit[@]}"} > "$source_c"
  fi
  "${CC:-cc}" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I "$work" "$source_c" verse/everglade/guard-native.c -lm -o "$work/test"
  "$work/test"
  printf 'guard %s passed\n' "$mode"
done
