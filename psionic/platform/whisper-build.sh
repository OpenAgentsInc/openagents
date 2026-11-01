#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles psionic/lib/whisper/search.coax (and the engine it loads) to C and links the host)
# Builds psionic-whisper into OUT (default psionic/build/whisper). On macOS it
# also compiles psionic/lib/whisper/device.coax to Metal and links the Metal
# host, so --gpu can run the encoder on the GPU.
#   psionic/platform/whisper-build.sh [OUT]
set -euo pipefail
cd "$(dirname "$0")/../.."
out=${1:-psionic/build/whisper}
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mkdir -p "$out"
"$coax" c psionic/lib/whisper/search.coax --release --library --header "$out/whisper.h" > "$out/whisper.c"
flags=(-std=c11 -O2 -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -I"$out")
if [[ $(uname) == Darwin ]]; then
  "$coax" metal-compute psionic/lib/whisper/device.coax --release > "$out/whisper.metal"
  { printf 'static const char whisper_metal_source[] = {\n'; xxd -i < "$out/whisper.metal"; printf ', 0};\n'; } > "$out/whisper-metal.h"
  "$cc" -fobjc-arc -mmacosx-version-min=15.0 -O2 -c psionic/platform/metal-host.m -o "$out/metal-host.o"
  "$cc" "${flags[@]}" -DPSIONIC_WHISPER_METAL -Ipsionic/platform \
    psionic/platform/whisper-face.c "$out/whisper.c" "$out/metal-host.o" -framework Metal -framework Foundation -lm -lpthread -o "$out/psionic-whisper"
else
  "$cc" "${flags[@]}" psionic/platform/whisper-face.c "$out/whisper.c" -lm -lpthread -o "$out/psionic-whisper"
fi
echo "$out/psionic-whisper"
