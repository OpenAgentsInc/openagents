#!/usr/bin/env bash
# Build our Coax Responses edge and keep original weights resident until exit.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ $# -lt 1 || $# -gt 2 ]]; then echo 'usage: psionic/serve-gptoss.sh ORIGINAL_CACHE [PORT=8080]' >&2; exit 2; fi
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
out=$(mktemp -d "${TMPDIR:-/tmp}/psionic-responses-server.XXXXXX")
trap 'rm -rf "$out"' EXIT
"$coax" c psionic/lib/responses/server-host.coax --library --release --run-steps 2000000 --header "$out/device-host.h" > "$out/device-host.c"
"$coax" cuda psionic/lib/gptoss/device-kernels.coax > "$out/device-kernels.cu"
libs=(-lm -pthread)
if [[ $(uname -s) == Linux ]]; then libs+=(-ldl); fi
"$cc" -std=c11 -O3 -g -ffp-contract=off -fno-fast-math -I"$out" -Ipsionic/platform \
    "$out/device-host.c" psionic/platform/cuda-host.c psionic/platform/gptoss-device.c \
    psionic/platform/responses-server.c "${libs[@]}" -o "$out/responses-server"
"$out/responses-server" "$1" psionic/fixtures/harmony/gptoss.hst "$out/device-kernels.cu" "${2:-8080}"
