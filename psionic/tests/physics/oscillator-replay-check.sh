#!/usr/bin/env bash
# Firm reason5: native build wrapper; C/Python evidence is independent reason2 tooling.
set -euo pipefail
case "$(uname -s):$(uname -m)" in
  Darwin:arm64) ;;
  *) printf 'oscillator replay qualification supports Darwin arm64 only\n' >&2;exit 1 ;;
esac
work="${1:?usage: oscillator-replay-check.sh OUTPUT_DIRECTORY}";mkdir -p "$work"
coax="${COAX:-coax/bin/coax}";compiler="${CC:-cc}";python="${PYTHON:-python3}"
for mode in debug release sanitized;do
  mkdir -p "$work/$mode-include"
  options=();flags=(-O0 -g)
  if [[ "$mode" == release ]];then options=(--release);flags=(-O2);fi
  if [[ "$mode" == sanitized ]];then flags=(-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer);fi
  "$coax" c psionic/lib/physics/oscillator-replay-codec.coax --library "${options[@]}" --header "$work/$mode-include/replay.h" > "$work/$mode.c"
  "$compiler" -std=c11 "${flags[@]}" -ffp-contract=off -fno-fast-math -I"$work/$mode-include" "$work/$mode.c" psionic/tests/physics/oscillator-replay-native.c -lm -o "$work/$mode"
  "$python" psionic/tests/physics/oscillator-replay-reference.py prepare "$work/$mode" "$work/$mode" "$work/$mode.c"
  "$work/$mode" "$work/$mode" "$work/$mode.ids" "$work/$mode.faults"
  "$python" psionic/tests/physics/oscillator-replay-reference.py verify "$work/$mode" "$work/$mode"
done
"$python" psionic/tests/physics/oscillator-replay-modes.py "$work"
