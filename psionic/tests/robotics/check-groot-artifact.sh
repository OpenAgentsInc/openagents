#!/usr/bin/env bash
# Firm reason: 5 (thin compiler wrapper); C/Python are independent oracles (2).
set -euo pipefail
cd "$(dirname "$0")/../../.."
work="${1:?usage: check-groot-artifact.sh OUTPUT_DIRECTORY}";mkdir -p "$work"
coax="${COAX:-$PWD/coax/bin/coax}";cc="${COAX_CC:-${CC:-cc}}"
for mode in debug release;do
  flags=();opt=-O0;if [[ $mode == release ]];then flags=(--release);opt=-O2;fi
  "$coax" c psionic/lib/robotics/groot-artifact.coax --library "${flags[@]}" --header "$work/groot-artifact.h" > "$work/$mode.c"
  "$cc" -std=c11 "$opt" -g -ffp-contract=off -fno-fast-math -I"$work" "$work/$mode.c" psionic/tests/robotics/groot-artifact-native.c -lm -o "$work/$mode"
  "$work/$mode" psionic/fixtures/robotics/groot-libero "$work/$mode.facts"
done
"$cc" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$work" "$work/debug.c" psionic/tests/robotics/groot-artifact-native.c -lm -o "$work/sanitized"
"$work/sanitized" psionic/fixtures/robotics/groot-libero "$work/sanitized.facts"
python3 -I psionic/tests/robotics/groot_artifact_reference.py "$work/debug.facts" "$work/release.facts" "$work/sanitized.facts"
