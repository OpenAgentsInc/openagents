#!/usr/bin/env bash
# Opt-in comparator: Psionic's projection kernels against cuBLAS on Elephant's
# shapes. cuBLAS is test-only and never part of check.sh, a library or a server;
# the harness skips when PSIONIC_CUBLAS (default ~/cublas-13.2) is absent.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-cublas.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" cuda psionic/lib/device/project.coax --release > "$out/project.cu"
"$cc" -std=c11 -O2 -Wall -Wextra -Werror -Wno-missing-field-initializers -Ipsionic/platform \
    psionic/platform/cuda-host.c psionic/tests/cublas_bench.c -ldl -lm -o "$out/cublas-bench"
LD_LIBRARY_PATH="${CUDA_HOME:-$HOME/cuda-13.2}/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" "$out/cublas-bench" "$out/project.cu"
