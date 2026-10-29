#!/usr/bin/env bash
# Real original 20b GPU path; no downloaded runtime, conversion or CPU fallback.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ $# != 2 ]]; then echo 'usage: psionic/run-gptoss.sh ORIGINAL_CACHE "prompt"|--interactive' >&2; exit 2; fi
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-gptoss-chat.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" c psionic/lib/gptoss/device-host.coax --library --release --run-steps 2000000 --header "$out/device-host.h" > "$out/device-host.c"
"$coax" cuda psionic/lib/gptoss/device-kernels.coax > "$out/device-kernels.cu"
libs=(-lm)
if [[ $(uname -s) == Linux ]]; then libs+=(-ldl); fi
"$cc" -std=c11 -O3 -ffp-contract=off -fno-fast-math -I"$out" -Ipsionic/platform \
    "$out/device-host.c" psionic/platform/cuda-host.c psionic/platform/gptoss-device.c \
    psionic/platform/gptoss-chat.c "${libs[@]}" -o "$out/gptoss-chat"
"$out/gptoss-chat" "$1" psionic/fixtures/harmony/gptoss.hst "$out/device-kernels.cu" "$2"
