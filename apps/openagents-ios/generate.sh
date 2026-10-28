#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
# Xcode exports an iOS SDKROOT; the bootstrap compiler runs on the build Mac.
# The explicit host triple also blocks inherited iOS deployment settings.
# Product output is ordinary C11, compiled by Xcode for the actual target SDK.
mkdir -p apps/openagents-ios/Generated
compiler=coax/bin/coax
if [[ ! -x "$compiler" || coax/build.sh -nt "$compiler" || apps/openagents-ios/generate.sh -nt "$compiler" ]] || [[ -n "$(find coax/src coax/lib coax/bootstrap -type f \( -name '*.cpp' -o -name '*.h' -o -name '*.c' -o -name '*.inc' -o -name '*.coax' \) -newer "$compiler" -print -quit)" ]]; then
  env -u CFLAGS -u CXXFLAGS SDKROOT="$(xcrun --sdk macosx --show-sdk-path)" \
    CXX="$(xcrun --sdk macosx --find clang++)" \
    COAX_HOST_TARGET="$(uname -m)-apple-macosx11.0" coax/build.sh
fi
out=apps/openagents-ios/Generated
for name in home player grid cast camera wallet haunt hud benchmarks; do
  # Home, Player, Grid, Cast, Camera, Wallet: one C library and header each. Wallet
  # is #authority, so its checks run in this compile and a failure stops it.
  base="$(printf '%s' "${name:0:1}" | tr a-z A-Z)${name:1}"
  debug_flags=()
  if [[ "${CONFIGURATION:-Debug}" == Debug ]]; then debug_flags+=(--debug-info --source-map "$out/$base.map.json"); fi
  "$compiler" c "metareal/$name.coax" --library --header "$out/$base.h" "${debug_flags[@]}" > "$out/$base.c.tmp"
  mv "$out/$base.c.tmp" "$out/$base.c"
done
"$compiler" c metareal/haunt-buffer.coax --library --header "$out/HauntBuffer.h" > "$out/HauntBuffer.c"
# Voice: the engine, its wire codecs and the floor/meter authorities, under
# names that cannot collide with metareal/platform/voice.h on a
# case-insensitive disk (so the engine is VoiceEngine, not Voice). Floor and
# meter are #authority: their checks run in this compile.
for pair in voice:VoiceEngine voice-wire:VoiceWire voice-floor:VoiceFloor voice-meter:VoiceMeter; do
  "$compiler" c "metareal/${pair%%:*}.coax" --library --header "$out/${pair##*:}.h" > "$out/${pair##*:}.c.tmp"
  mv "$out/${pair##*:}.c.tmp" "$out/${pair##*:}.c"
done
"$compiler" metal metareal/light.coax > "$out/HomeShaders.txt.tmp"
"$compiler" metal metareal/haunt-light.coax >> "$out/HomeShaders.txt.tmp"
cat apps/openagents-ios/OpenAgents/Platform/Stages.metal >> "$out/HomeShaders.txt.tmp"
mv "$out/HomeShaders.txt.tmp" "$out/HomeShaders.txt"
