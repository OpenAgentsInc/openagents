#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles psionic/lib/whisper/search.coax (and the engine it loads) to C and links the host)
# Builds psionic-whisper and libpsionic-whisper.a (the library
# psionic/platform/psionic-stt.h declares) into OUT (default
# psionic/build/whisper). On macOS it also compiles
# psionic/lib/whisper/device.coax to Metal and links the Metal host, so
# --gpu can run the encoder on the GPU.
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
  flags+=(-DPSIONIC_WHISPER_METAL)
  extra=("$out/metal-host.o")
  link=(-framework Metal -framework Foundation)
else
  extra=(); link=()
fi
flags+=(-Ipsionic/platform)
"$cc" "${flags[@]}" -c "$out/whisper.c" -o "$out/whisper.o"
"$cc" "${flags[@]}" -c psionic/platform/whisper-face.c -o "$out/whisper-face.o"
"$cc" "${flags[@]}" -DPSIONIC_WHISPER_LIBRARY -c psionic/platform/whisper-face.c -o "$out/psionic-stt.o"
"$cc" "$out/whisper-face.o" "$out/whisper.o" ${extra[@]+"${extra[@]}"} ${link[@]+"${link[@]}"} -lm -lpthread -o "$out/psionic-whisper"
rm -f "$out/libpsionic-whisper.a"
ar rcs "$out/libpsionic-whisper.a" "$out/psionic-stt.o" "$out/whisper.o" ${extra[@]+"${extra[@]}"}
echo "$out/psionic-whisper"
