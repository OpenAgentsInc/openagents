#!/usr/bin/env bash
# Warp kernels on a CUDA device: the tensor-core MMA primitive against its Coax
# meaning, and the tensor-core projection against the CPU reference profile.
# Prints SKIP without a CUDA driver or NVRTC. Usage: psionic/tests/run_device_warp.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-device-warp.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" cuda psionic/lib/device/warp.coax --release > "$out/warp.cu"
"$coax" cuda psionic/lib/device/project.coax --release > "$out/project.cu"
"$coax" c psionic/lib/device/warp.coax --library --release --header "$out/warp.h" > "$out/warp.c"
"$coax" c psionic/lib/qwen-matvec.coax --library --release --header "$out/qwen-matvec.h" > "$out/qwen-matvec.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function \
    -Wno-unused-parameter -Wno-missing-field-initializers -I"$out" -Ipsionic/platform \
    "$out/warp.c" "$out/qwen-matvec.c" psionic/platform/cuda-host.c psionic/tests/device_warp.c -ldl -lm -o "$out/device-warp"
"$out/device-warp" "$out/warp.cu" "$out/project.cu"
