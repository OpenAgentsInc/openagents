#!/usr/bin/env bash
# Chapter 32: group kernels (threadgroup memory and barriers) on the GPU
# against the same Coax on the CPU, bit for bit. On macOS the Apple GPU runs
# coax metal-compute's output; elsewhere a CUDA device runs coax cuda's.
# Prints SKIP without a device. Usage: psionic/tests/run_group_kernels.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-group.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" test psionic/tests/group-matmul.coax
"$coax" c psionic/tests/group-matmul.coax --library --release --header "$out/group-matmul.h" > "$out/group-matmul.c"
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -Werror -Wno-unknown-pragmas -Wno-unused-function -Wno-unused-parameter -Wno-missing-field-initializers -Wno-misleading-indentation)
if [[ $(uname) == Darwin ]]; then
    "$coax" metal-compute psionic/tests/group-matmul.coax --release > "$out/group-matmul.metal"
    "$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -Wall -Wextra -Werror -c psionic/platform/metal-host.m -o "$out/metal-host.o"
    "$cc" "${flags[@]}" -DGROUP_METAL -I"$out" -Ipsionic/platform "$out/group-matmul.c" psionic/tests/group_kernels.c "$out/metal-host.o" \
        -framework Metal -framework Foundation -lm -o "$out/group-kernels"
    "$out/group-kernels" "$out/group-matmul.metal"
else
    "$coax" cuda psionic/tests/group-matmul.coax --release > "$out/group-matmul.cu"
    "$cc" "${flags[@]}" -I"$out" -Ipsionic/platform "$out/group-matmul.c" psionic/platform/cuda-host.c psionic/tests/group_kernels.c -ldl -lm -o "$out/group-kernels"
    "$out/group-kernels" "$out/group-matmul.cu"
fi
