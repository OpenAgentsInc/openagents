#!/usr/bin/env bash
# Firm reason2/5: opt-in targeted recorded-control SDK integration, no NN.
set -euo pipefail
xml="${1:?usage: check-panda-clock-prefix.sh XML INIT TRACE SCENARIO NEW_DIRECTORY}";initial="${2:?}";trace="${3:?}";scenario="${4:?}";work="${5:?}"
[[ ! -e "$work" ]] || { echo 'new output directory required' >&2;exit 2; };mkdir -p "$work"
coax="${COAX:-coax/bin/coax}";cc="${CC:-cc}";python="${MUJOCO_REFERENCE_PYTHON:-$HOME/.cache/psionic/groot-reference-env/bin/python}"
sdk="${MUJOCO_ROOT:-$("$python" -c 'import pathlib,importlib.util;print(pathlib.Path(importlib.util.find_spec("mujoco").origin).parent)')}"
"$python" psionic/tests/robotics/panda_clock_prefix_reference.py "$xml" "$initial" "$trace" "$scenario" "$work/result" --authenticate-only
"$coax" c psionic/tests/robotics/panda-clock-session.coax --library --header "$work/panda-session.h" > "$work/panda-session.c"
"$cc" -std=c11 -O1 -g -ffp-contract=off -fno-fast-math -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer -Wno-deprecated-declarations -I"$work" -Ipsionic/platform -I"$sdk/include" "$work/panda-session.c" psionic/tests/robotics/panda-clock-prefix.c psionic/platform/panda-env.c "$sdk/libmujoco.3.3.1.dylib" -framework OpenGL -lm -o "$work/prefix"
export DYLD_LIBRARY_PATH="$sdk${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
"$work/prefix" "$xml" "$initial" "$trace" "$scenario" "$work/result"
"$python" psionic/tests/robotics/panda_clock_prefix_reference.py "$xml" "$initial" "$trace" "$scenario" "$work/result" --record "$work/reference.json"
