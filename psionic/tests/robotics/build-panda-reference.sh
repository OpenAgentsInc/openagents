#!/usr/bin/env bash
# Firm reason5: opt-in reference bridge compiler wrapper, not a shipped runtime.
set -euo pipefail
work="${1:?usage: build-panda-reference.sh BUILD [QUALIFIED_GENERATED_C_CACHE]}"
cache="${2:-}"
mkdir -p "$work"
cc="${CC:-cc}"
python="${GROOT_REFERENCE_PYTHON:-$HOME/.cache/psionic/groot-reference-env/bin/python}"
sdk="${MUJOCO_ROOT:-$("$python" -c 'import importlib.util,pathlib;print(pathlib.Path(importlib.util.find_spec("mujoco").origin).parent)')}"
if [[ -n "$cache" ]];then
  cp "$cache/panda-session.c" "$cache/panda-session.h" "$cache/panda-policy-generated.c" "$cache/panda-policy-generated.h" "$work/"
else
  coax="${COAX:-coax/bin/coax}"
  "$coax" c psionic/tests/robotics/panda-session.coax --library --release --header "$work/panda-session.h" > "$work/panda-session.c"
  "$coax" c psionic/lib/robotics/panda-policy.coax --library --release --header "$work/panda-policy-generated.h" > "$work/panda-policy-generated.c"
fi
"$cc" -std=c11 -O2 -dynamiclib -ffp-contract=off -fno-fast-math -Wno-deprecated-declarations -I"$work" -I"$sdk/include" -Ipsionic/platform -Ipsionic/tests/robotics "$work/panda-session.c" "$work/panda-policy-generated.c" psionic/tests/robotics/panda_reference_bridge.c psionic/tests/robotics/panda_reference_loop.c psionic/platform/panda-env.c -L"$sdk" -lmujoco.3.3.1 -framework OpenGL -lm -o "$work/panda-reference.dylib"
printf 'Built opt-in reference bridge %s/panda-reference.dylib\n' "$work"
