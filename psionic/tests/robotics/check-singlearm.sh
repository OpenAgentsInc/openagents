#!/usr/bin/env bash
# Firm reason: 5 (thin compiler wrapper); C is independent oracle (2).
set -euo pipefail
cd "$(dirname "$0")/../../.."
mkdir -p "$1"
coax="${COAX:-$PWD/coax/bin/coax}"
for mode in debug release; do
  flags=();opt=-O0
  if [[ "$mode" == release ]]; then flags=(--release);opt=-O2;fi
  "$coax" c psionic/lib/robotics/singlearm.coax --library "${flags[@]}" --header "$1/singlearm.h" > "$1/singlearm-$mode.c"
  "${CC:-cc}" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -I"$1" "$1/singlearm-$mode.c" psionic/tests/robotics/singlearm-native.c -lm -o "$1/singlearm-$mode"
  "$1/singlearm-$mode"
done
"${CC:-cc}" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$1" "$1/singlearm-debug.c" psionic/tests/robotics/singlearm-native.c -lm -o "$1/singlearm-sanitized"
"$1/singlearm-sanitized"
