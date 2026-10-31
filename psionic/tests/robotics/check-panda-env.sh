#!/usr/bin/env bash
# Firm reason5 compiler wrapper; C/NumPy are independent reason2 oracles.
set -euo pipefail
work="${1:?usage: check-panda-env.sh OUTPUT_DIRECTORY}";mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
"$coax" c psionic/lib/robotics/panda-env.coax --library --header "$work/panda-env-generated.h" > "$work/debug.c"
"${CC:-cc}" -std=c11 -O0 -g -ffp-contract=off -fno-fast-math -I"$work" "$work/debug.c" psionic/tests/robotics/panda-env-native.c -lm -o "$work/debug"
"$work/debug"
"$coax" c psionic/lib/robotics/panda-env.coax --library --release --header "$work/panda-env-generated.h" > "$work/release.c"
"${CC:-cc}" -std=c11 -O2 -ffp-contract=off -fno-fast-math -I"$work" "$work/release.c" psionic/tests/robotics/panda-env-native.c -lm -o "$work/release"
"$work/release"
"${CC:-cc}" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -I"$work" "$work/debug.c" psionic/tests/robotics/panda-env-native.c -lm -o "$work/sanitized"
"$work/sanitized"
if [[ -n "${PSIONIC_NUMPY_PYTHON:-}" ]]; then
  shared=(-shared -fPIC);if [[ "$(uname -s)" == Darwin ]];then shared=(-dynamiclib);fi
  "${CC:-cc}" -std=c11 -O0 -ffp-contract=off -fno-fast-math "${shared[@]}" "$work/debug.c" -lm -o "$work/panda-env.so"
  "$PSIONIC_NUMPY_PYTHON" psionic/tests/robotics/panda_env_reference.py "$work/panda-env.so"
fi
