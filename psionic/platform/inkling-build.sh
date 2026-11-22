#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax metal-compute and clang; holds no logic)
# Builds the native Psionic Metal runtime for Inkling 2 Mini:
#   - inkling-kernels.metal (emitted from psionic/lib/inkling/kernels.coax via coax metal-compute)
#   - psionic-inkling-serve (local HTTP /v1/chat/completions & /v1/responses server on Metal)
#   - psionic-inkling-cli   (standalone CLI runner on Metal)
# Usage: psionic/platform/inkling-build.sh [OUT_DIR]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
coax="${COAX:-$repo/coax/bin/coax}"
out="${1:-$repo/psionic/build}"
mkdir -p "$out/obj"

"$coax" metal-compute "$repo/psionic/lib/inkling/kernels.coax" --release > "$out/inkling-kernels.metal"
"$coax" c "$repo/psionic/lib/inkling/json-schema.coax" --release --library --header "$out/obj/json-schema.h" > "$out/obj/json-schema.c"

cflags=(-mmacosx-version-min=14.0 -O2 -ffp-contract=off -fno-fast-math -Wall -Wextra -I"$here" -I"$out/obj")
/usr/bin/clang -fobjc-arc -mmacosx-version-min=14.0 -O2 -I"$here" -c "$here/metal-host.m" -o "$out/obj/metal-host.o"
/usr/bin/clang -std=c11 "${cflags[@]}" -c "$here/inkling-gguf.c" -o "$out/obj/inkling-gguf.o"
/usr/bin/clang -std=c11 "${cflags[@]}" -c "$here/inkling-metal.c" -o "$out/obj/inkling-metal.o"
/usr/bin/clang -std=c11 -mmacosx-version-min=14.0 -O2 -Wno-unused-function -Wno-unused-variable -c "$out/obj/json-schema.c" -o "$out/obj/json-schema.o"
/usr/bin/clang -std=c11 "${cflags[@]}" -c "$here/inkling-serve.c" -o "$out/obj/inkling-serve.o"
/usr/bin/clang -std=c11 "${cflags[@]}" -c "$here/inkling-cli.c" -o "$out/obj/inkling-cli.o"

/usr/bin/clang -mmacosx-version-min=14.0 -O2 \
  "$out/obj/metal-host.o" "$out/obj/inkling-gguf.o" "$out/obj/inkling-metal.o" "$out/obj/json-schema.o" "$out/obj/inkling-serve.o" \
  -framework Metal -framework Foundation -lm -lpthread -o "$out/psionic-inkling-serve"

/usr/bin/clang -mmacosx-version-min=14.0 -O2 \
  "$out/obj/metal-host.o" "$out/obj/inkling-gguf.o" "$out/obj/inkling-metal.o" "$out/obj/inkling-cli.o" \
  -framework Metal -framework Foundation -lm -o "$out/psionic-inkling-cli"

echo "built $out/psionic-inkling-serve, $out/psionic-inkling-cli, and $out/inkling-kernels.metal"
