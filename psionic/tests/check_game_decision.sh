#!/usr/bin/env bash
# Focused game ticket gate: no models, relays or unrelated oracle setup.
set -euo pipefail
cd "$(dirname "$0")/../.."
native_only=false mutate=false jobs=4
while (($#)); do
  case "$1" in
    --native-only) native_only=true;shift ;;
    --mutate) mutate=true;shift ;;
    --jobs) jobs="$2";shift 2 ;;
    *) echo "usage: check_game_decision.sh [--native-only] [--mutate] [--jobs N]" >&2;exit 2 ;;
  esac
done
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc}
if ! $native_only; then
  "$coax" test psionic/lib/game/decision.coax --library
  if $mutate; then "$coax" mutate psionic/lib/game/decision.coax --root-only --jobs "$jobs";fi
fi
work=$(mktemp -d);trap 'rm -rf "$work"' EXIT
for mode in debug release; do
  args=();opt=-O0
  if [[ "$mode" == release ]];then args+=(--release);opt=-O2;fi
  "$coax" c psionic/lib/game/decision.coax --library "${args[@]}" --header "$work/decision.h" > "$work/decision.c"
  for native in plain sanitized;do
    flags=(-std=c11 "$opt" -g -Wall -Wextra -Werror -Wno-unused-function -Wno-unused-variable)
    if [[ "$native" == sanitized ]];then flags+=(-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
    "$cc" "${flags[@]}" -I"$work" "$work/decision.c" psionic/tests/game_decision.c -lm -o "$work/check"
    "$work/check"
    echo "game decision: $mode/$native passed"
  done
done
