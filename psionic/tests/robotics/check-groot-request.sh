#!/usr/bin/env bash
# Firm reason5: compiler wrapper; independent C/original-tokenizer oracle is reason2.
set -euo pipefail
work="${1:?usage: check-groot-request.sh OUTPUT_DIRECTORY}"
mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
original=()
if [[ -n "${PSIONIC_COSMOS_PST:-}" ]]; then original=("$PSIONIC_COSMOS_PST"); fi
"$coax" c psionic/lib/robotics/groot-request.coax --library --header "$work/groot-request.h" > "$work/debug.c"
"${CC:-cc}" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/debug.c" psionic/tests/robotics/groot-request-native.c -lm -o "$work/debug"
"$work/debug" "${original[@]}"
"$coax" c psionic/lib/robotics/groot-request.coax --library --release --header "$work/groot-request.h" > "$work/release.c"
"${CC:-cc}" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/release.c" psionic/tests/robotics/groot-request-native.c -lm -o "$work/release"
"$work/release" "${original[@]}"
"${CC:-cc}" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$work" "$work/debug.c" psionic/tests/robotics/groot-request-native.c -lm -o "$work/sanitized"
"$work/sanitized" "${original[@]}"
if [[ -n "${PSIONIC_NUMPY_PYTHON:-}" ]]; then
  shared=(-shared -fPIC)
  if [[ "$(uname -s)" == Darwin ]]; then shared=(-dynamiclib); fi
  "${CC:-cc}" -std=c11 -O0 -ffp-contract=off -fno-fast-math "${shared[@]}" "$work/debug.c" -lm -o "$work/request.so"
  "$PSIONIC_NUMPY_PYTHON" psionic/tests/robotics/groot_request_reference.py --library "$work/request.so"
fi
