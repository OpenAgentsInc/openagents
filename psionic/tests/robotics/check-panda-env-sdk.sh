#!/usr/bin/env bash
# Firm reason5: compiler/SDK effect wrapper; independent reference is reason2.
set -euo pipefail
xml="${1:?usage: check-panda-env-sdk.sh SOURCE_SCENE_XML INIT45F64 NEW_OUTPUT_DIRECTORY}"
initial="${2:?}";work="${3:?}"
[[ ! -e "$work" ]] || { echo 'output already exists' >&2;exit 2; };mkdir -p "$work"
coax="${COAX:-coax/bin/coax}"
sdk="${MUJOCO_ROOT:-$HOME/.cache/psionic/groot-reference-env/lib/python3.12/site-packages/mujoco}"
python="${MUJOCO_REFERENCE_PYTHON:-$HOME/.cache/psionic/groot-reference-env/bin/python}"
export DYLD_LIBRARY_PATH="$sdk${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
"$coax" c psionic/lib/robotics/panda-env.coax --library --header "$work/panda-env-generated.h" > "$work/env.c"
"$coax" c psionic/lib/robotics/panda-control.coax --library --header "$work/panda-control.h" > "$work/control.c"
"$coax" c psionic/lib/robotics/panda-env.coax --library --release --header "$work/panda-env-generated.h" > "$work/env-release.c"
"$coax" c psionic/lib/robotics/panda-control.coax --library --release --header "$work/panda-control.h" > "$work/control-release.c"
common=(-std=c11 -ffp-contract=off -fno-fast-math -Wno-deprecated-declarations -I"$work" -Ipsionic/platform -I"$sdk/include")
effects=(psionic/platform/panda-env.c psionic/tests/robotics/panda-env-sdk-native.c "$sdk/libmujoco.3.3.1.dylib" -framework OpenGL -lm)
"${CC:-cc}" "${common[@]}" -O0 -g "$work/env.c" "$work/control.c" "${effects[@]}" -o "$work/debug"
"$work/debug" "$xml" "$initial" "$work/debug"
"${CC:-cc}" "${common[@]}" -O2 "$work/env-release.c" "$work/control-release.c" "${effects[@]}" -o "$work/release"
"$work/release" "$xml" "$initial" "$work/release"
"${CC:-cc}" "${common[@]}" -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer "$work/env.c" "$work/control.c" "${effects[@]}" -o "$work/sanitized"
"$work/sanitized" "$xml" "$initial" "$work/sanitized"
for mode in release sanitized;do
  for suffix in initial-telemetry.f64 stale-telemetry.f64 fresh-telemetry.f64 observation.f32 states.f64 controls.f64;do
    cmp "$work/debug.$suffix" "$work/$mode.$suffix"
  done
done
"${CC:-cc}" "${common[@]}" -O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer psionic/platform/panda-env.c psionic/tests/robotics/panda-env-capture.c "$sdk/libmujoco.3.3.1.dylib" -framework OpenGL -o "$work/capture"
"$work/capture" "$xml" "$initial" "$work/render" --render
"$python" psionic/tests/robotics/panda_env_sdk_reference.py "$xml" "$initial" "$work/debug" --render "$work/render" --record "$work/reference.json"
