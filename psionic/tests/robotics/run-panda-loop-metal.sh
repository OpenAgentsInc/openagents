#!/usr/bin/env bash
# Firm reason5: opt-in build/effects wrapper, no model/controller arithmetic.
# Requires separately acquired/authenticated original files and SDK3.3.1.
set -euo pipefail
shards="${1:?usage: run-panda-loop-metal.sh SHARDS PST NOISE_F32 PAIRS OUTPUT [TICKS] [SCENARIO] [DEMO_IDS...]}"
pst="${2:?PST required}"
noise="${3:?explicit5280F32 noise artifact required}"
pairs="${4:?private evaluation pair directory required}"
output="${5:?new output directory required}"
if [[ -e "$output" ]];then echo "Panda output already exists; choose a new output directory: $output" >&2;exit 2;fi
mkdir "$output"
ticks="${6:-720}"
scenario="${7:-0}"
shift "$(( $# < 7 ? $# : 7 ))"
ids=("$@")
if [[ ${#ids[@]} == 0 ]];then ids=(001);fi
coax="${COAX:-coax/bin/coax}"
compiler="${CC:-cc}"
python="${GROOT_REFERENCE_PYTHON:-$HOME/.cache/psionic/groot-reference-env/bin/python}"
sdk="${MUJOCO_ROOT:-$("$python" -c 'import importlib.util,pathlib;print(pathlib.Path(importlib.util.find_spec("mujoco").origin).parent)')}"
build="$output/build"
mkdir -p "$build/shaders"
if [[ -n "${PANDA_QUERY_SOURCE:-}" ]];then
  "$coax" c psionic/tests/robotics/panda-query-runtime.coax --library --release --header "$build/panda-session.h" > "$build/panda-session.c"
else
"$coax" c psionic/tests/robotics/panda-clock-session.coax --library --release --header "$build/panda-session.h" > "$build/panda-session.c"
fi
"$coax" c psionic/lib/robotics/panda-policy.coax --library --release --header "$build/panda-policy-generated.h" > "$build/panda-policy-generated.c"
if [[ -n "${PANDA_SHADER_CACHE:-}" ]];then
  for module in tile device vision wide text action;do cp "$PANDA_SHADER_CACHE/$module.metal" "$build/shaders/$module.metal";done
else
  sources=(psionic/lib/device/tile.coax psionic/lib/robotics/groot-device-kernels.coax psionic/lib/robotics/groot-vision-device.coax psionic/lib/robotics/groot-action-wide-kernels.coax psionic/lib/robotics/groot-text-kernels.coax psionic/lib/robotics/groot-action-metal-kernels.coax)
  names=(tile device vision wide text action)
  for i in 0 1 2 3 4 5;do "$coax" metal-compute "${sources[$i]}" --release > "$build/shaders/${names[$i]}.metal";done
fi
"$compiler" -fobjc-arc -mmacosx-version-min=15.0 -O2 -c psionic/platform/metal-host.m -o "$build/metal-host.o"
flags=(-std=c11 -ffp-contract=off -fno-fast-math -Wno-deprecated-declarations)
case "${PANDA_NATIVE_MODE:-release}" in
  debug) flags+=(-O0 -g);;
  sanitize) flags+=(-O1 -g -fsanitize=address,undefined);;
  release) flags+=(-O2);;
  *) echo "PANDA_NATIVE_MODE must be debug,release,sanitize" >&2;exit 2;;
esac
if [[ -n "${PANDA_QUERY_SOURCE:-}" ]];then
  flags+=(-DPANDA_OWNED_QUERY_V1=1 -D_DARWIN_C_SOURCE -fno-builtin-sin -fno-builtin-cos)
  if [[ "${PANDA_NATIVE_MODE:-release}" == sanitize ]];then flags+=(-fno-sanitize-recover=all -fno-omit-frame-pointer);fi
fi
"$compiler" "${flags[@]}" -I"$build" -I"$sdk/include" -Ipsionic/platform -Ipsionic/tests/robotics "$build/panda-session.c" "$build/panda-policy-generated.c" psionic/platform/panda-policy.c psionic/platform/panda-env.c psionic/tests/robotics/panda_loop_metal.c "$build/metal-host.o" -L"$sdk" -lmujoco.3.3.1 -framework Metal -framework Foundation -framework OpenGL -lm -o "$build/panda-loop"
cases=()
for id in "${ids[@]}";do [[ "$id" =~ ^[0-9][0-9][0-9]$ ]] || exit 2;cases+=("$pairs/demo_$id");done
export DYLD_LIBRARY_PATH="$sdk${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
if [[ "${PANDA_BUILD_ONLY:-0}" == 1 ]];then echo "Panda build only: no policy/SDK open, NN or rendering";exit 0;fi
"$build/panda-loop" "$shards" "$pst" "$noise" "$build/shaders" "$output" 'pick up the book and place it in the back compartment of the caddy' "$ticks" "$scenario" "${cases[@]}"
