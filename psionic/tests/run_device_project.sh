#!/usr/bin/env bash
# Chapter 37: the Coax projection kernels on a CUDA device, against the same
# kernels compiled to C and the CPU reference profile. Without a CUDA driver
# or NVRTC the harness prints SKIP and exits 0. Usage:
#   psionic/tests/run_device_project.sh [quick]
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-device-project.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" test psionic/lib/device/project.coax
"$coax" test psionic/lib/device/project.coax --release
"$coax" mutate psionic/lib/device/project.coax
"$coax" cuda psionic/lib/device/project.coax --release > "$out/project.cu"
"$coax" c psionic/lib/device/project.coax --library --release --header "$out/project.h" > "$out/project.c"
"$coax" c psionic/lib/qwen-matvec.coax --library --release --header "$out/qwen-matvec.h" > "$out/qwen-matvec.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function \
    -Wno-unused-parameter -Wno-missing-field-initializers -I"$out" -Ipsionic/platform \
    "$out/project.c" "$out/qwen-matvec.c" psionic/platform/cuda-host.c psionic/tests/device_project.c -ldl -lm -o "$out/device-project"
"$out/device-project" "$out/project.cu" "$@"
