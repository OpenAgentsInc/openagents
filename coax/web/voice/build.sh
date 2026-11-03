#!/usr/bin/env bash
# The browser's voice engine: verse/voice*.coax as C (one library each, named
# as verse/voice-host/build.sh names them), verse/platform/voice.c and the
# browser's face voice-web.c, linked into one wasm32 reactor that owns its
# memory. coax/web/voice.js loads it.
#   coax/web/voice/build.sh OUT.wasm
set -euo pipefail
out="$1"
repo="$(cd "$(dirname "$0")/../../.." && pwd)"
# The Wasm toolchain wasm.sh uses (toolchain.sh): Homebrew's on a Mac, nixpkgs' on Linux;
# COAXIAL_CLANG, COAXIAL_SYSROOT and COAXIAL_RESOURCES override it.
source "$repo/coax/web/toolchain.sh"
clang="$COAXIAL_CLANG"; sysroot="$COAXIAL_SYSROOT"; resources="$COAXIAL_RESOURCES"
work="$(dirname "$out")/.voice"
mkdir -p "$work"
cflags=(--target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" -std=c11 -O2
        -ffp-contract=off -fno-fast-math -w -I"$work" -I"$repo/verse/platform")
objects=()
for src in "$repo"/verse/voice.coax "$repo"/verse/voice-wire.coax "$repo"/verse/voice-floor.coax "$repo"/verse/voice-meter.coax; do
  name="$(basename "$src" .coax)"
  base="$(printf '%s' "$name" | awk -F- '{for(i=1;i<=NF;i++) printf toupper(substr($i,1,1)) substr($i,2)}')"
  [[ "$base" == "Voice" ]] && base=VoiceEngine
  (cd "$repo" && coax/bin/coax c "${src#"$repo"/}" --library --header "$work/$base.h") > "$work/$base.c"   # repo-relative: bytes don't depend on the checkout
  "$clang" "${cflags[@]}" -c "$work/$base.c" -o "$work/$base.o"
  objects+=("$work/$base.o")
done
"$clang" "${cflags[@]}" -c "$repo/verse/platform/voice.c" -o "$work/voice.o"
"$clang" "${cflags[@]}" -c "$repo/coax/web/voice/voice-web.c" -o "$work/voice-web.o"
"$clang" --target=wasm32-wasip1 --sysroot="$sysroot" -resource-dir="$resources" -O2 -mexec-model=reactor \
  -o "$out.tmp" "${objects[@]}" "$work/voice.o" "$work/voice-web.o" \
  -Wl,--max-memory=16777216,-z,stack-size=262144,--gc-sections,--strip-debug
mv "$out.tmp" "$out"
