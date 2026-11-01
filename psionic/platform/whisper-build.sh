#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles psionic/lib/whisper/search.coax (and the engine it loads) to C and links the host)
# Builds psionic-whisper into OUT (default psionic/build/whisper).
#   psionic/platform/whisper-build.sh [OUT]
set -euo pipefail
cd "$(dirname "$0")/../.."
out=${1:-psionic/build/whisper}
coax=${COAX:-$PWD/coax/bin/coax}
cc=${COAX_CC:-cc}
mkdir -p "$out"
"$coax" c psionic/lib/whisper/search.coax --release --library --header "$out/whisper.h" > "$out/whisper.c"
"$cc" -std=c11 -O2 -ffp-contract=off -fno-fast-math -Wno-unused-function -Wno-unused-parameter -Wno-unknown-pragmas -I"$out" \
  psionic/platform/whisper-face.c "$out/whisper.c" -lm -lpthread -o "$out/psionic-whisper"
echo "$out/psionic-whisper"
