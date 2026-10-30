#!/usr/bin/env bash
# Generated CUDA only; --compile-only needs NVRTC but no device/context.
set -euo pipefail
cd "$(dirname "$0")/../.."
coax=${COAX:-$PWD/coax/bin/coax};cc=${COAX_CC:-cc}
mode=${1:-run};if [[ $mode != run && $mode != --compile-only ]];then echo 'usage: run_deepseek_prefill_cuda.sh [--compile-only]' >&2;exit 2;fi
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-prefill-cuda.XXXXXX");trap 'rm -rf "$out"' EXIT
nvrtc=${PSIONIC_NVRTC:-}
if [[ -z $nvrtc ]];then for candidate in /nix/store/*-cuda*nvrtc*-lib/lib/libnvrtc.so;do if [[ -f $candidate ]];then nvrtc=$candidate;break;fi;done;fi
"$coax" cuda psionic/lib/deepseek/prefill-kernels.coax --release > "$out/kernels.cu"
if [[ $mode == --compile-only ]];then
    [[ -n $nvrtc ]] || { echo 'Set PSIONIC_NVRTC to the NVRTC library' >&2;exit 2; }
    "$cc" -std=c11 -O2 -Wall -Wextra -Werror psionic/tests/deepseek_prefill_nvrtc.c -ldl -o "$out/compile"
    "$out/compile" "$nvrtc" "$out/kernels.cu" "$out/kernels.ptx"
else
    "$coax" c psionic/lib/deepseek/prefill-kernels.coax --library --release --header "$out/kernels.h" > "$out/kernels.c"
    "$cc" -std=c11 -O3 -g -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -I"$out" -Ipsionic/platform "$out/kernels.c" psionic/platform/cuda-host.c psionic/tests/deepseek_prefill_device.c -ldl -lm -o "$out/device"
    args=();if [[ -n ${PSIONIC_DEEPSEEK_ROUTED_PACKET:-} ]];then args+=("$PSIONIC_DEEPSEEK_ROUTED_PACKET");fi
    PSIONIC_NVRTC="$nvrtc" "$out/device" "$out/kernels.cu" "${args[@]}"
fi
