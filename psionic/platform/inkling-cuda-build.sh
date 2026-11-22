#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax cuda and a C compiler; holds no logic)
# Builds the native Psionic CUDA runtime for Inkling 2 Mini on Linux, or
# cross-compiles it for Windows:
#   - inkling-kernels-cuda.cu (emitted from psionic/lib/inkling/kernels-cuda.coax via coax cuda)
#   - psionic-inkling-serve   (local HTTP /v1/chat/completions & /v1/responses server on CUDA)
#   - psionic-inkling-cli     (standalone CLI runner on CUDA)
# The build needs no CUDA toolkit: the driver and NVRTC are opened at run time.
# Usage: psionic/platform/inkling-cuda-build.sh [OUT_DIR]
#        TARGET=windows CC=x86_64-w64-mingw32-gcc psionic/platform/inkling-cuda-build.sh [OUT_DIR]
# LDFLAGS is passed to the link, for a toolchain whose libraries are not on its default path.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
target="${TARGET:-linux}"
out="${1:-$repo/psionic/build}"
mkdir -p "$out/obj"

"$coax" cuda "$repo/psionic/lib/inkling/kernels-cuda.coax" --release > "$out/inkling-kernels-cuda.cu"
"$coax" c "$repo/psionic/lib/inkling/json-schema.coax" --release --library --header "$out/obj/json-schema.h" > "$out/obj/json-schema.c"

cflags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -I"$here" -I"$out/obj")
if [ "$target" = windows ]; then
  cc="${CC:-x86_64-w64-mingw32-gcc}"
  exe=.exe
  libs=(-lws2_32 -lm)
else
  cc="${CC:-cc}"
  exe=
  cflags+=(-D_POSIX_C_SOURCE=200809L)
  libs=(-ldl -lpthread -lm)
fi

"$cc" -std=c11 -O2 -Wno-unused-function -Wno-unused-variable -c "$out/obj/json-schema.c" -o "$out/obj/json-schema.o"
for unit in cuda-host inkling-gguf inkling-cuda inkling-serve inkling-cli; do
  "$cc" "${cflags[@]}" -c "$here/$unit.c" -o "$out/obj/$unit.o"
done

"$cc" "$out/obj/cuda-host.o" "$out/obj/inkling-gguf.o" "$out/obj/inkling-cuda.o" "$out/obj/json-schema.o" "$out/obj/inkling-serve.o" \
  ${LDFLAGS:-} "${libs[@]}" -o "$out/psionic-inkling-serve$exe"
"$cc" "$out/obj/cuda-host.o" "$out/obj/inkling-gguf.o" "$out/obj/inkling-cuda.o" "$out/obj/inkling-cli.o" \
  ${LDFLAGS:-} "${libs[@]}" -o "$out/psionic-inkling-cli$exe"

echo "built $out/psionic-inkling-serve$exe, $out/psionic-inkling-cli$exe, and $out/inkling-kernels-cuda.cu"
