#!/usr/bin/env bash
# Builds the voice host and the dev client into metareal/voice-host/build/.
#   metareal/voice-host/build.sh
# The Coax engine (metareal/voice*.coax) becomes one C library per file with
# coax/bin/coax, exactly as apps/openagents-ios/generate.sh does for the app;
# metareal/platform/voice.c is the C face over it. libcurl comes from the
# macOS SDK. #authority files run their checks inside `coax c`, so a failing
# check stops the build.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
cd "$root"
sdk="$(xcrun --sdk macosx --show-sdk-path)"
cc="$(xcrun --sdk macosx --find clang)"
compiler=coax/bin/coax
if [[ ! -x "$compiler" ]] || [[ -n "$(find coax/src coax/lib coax/bootstrap -type f -newer "$compiler" -print -quit)" ]]; then
  env -u CFLAGS -u CXXFLAGS SDKROOT="$sdk" CXX="$(xcrun --sdk macosx --find clang++)" \
    COAX_HOST_TARGET="$(uname -m)-apple-macosx11.0" coax/build.sh
fi
out="$here/build"
gen="$out/gen"
mkdir -p "$gen"
engine=()
shopt -s nullglob
# The engine, plus the host's own reply-path decisions (voice-host/voice-reply.coax).
for src in metareal/voice*.coax metareal/voice-host/*.coax; do
  name="$(basename "$src" .coax)"                                    # voice-wire
  base="$(printf '%s' "$name" | awk -F- '{for(i=1;i<=NF;i++) printf toupper(substr($i,1,1)) substr($i,2)}')"  # VoiceWire
  # voice.coax -> VoiceEngine.h: "Voice.h" would be voice.h on a case-insensitive disk.
  [[ "$base" == "Voice" ]] && base=VoiceEngine
  "$compiler" c "$src" --library --header "$gen/$base.h" > "$gen/$base.c.tmp.$$"
  mv "$gen/$base.c.tmp.$$" "$gen/$base.c"
  engine+=("$gen/$base.c")
done
if [[ ${#engine[@]} -eq 0 || ! -f metareal/platform/voice.c ]]; then
  echo "build.sh: the voice engine (metareal/voice*.coax, metareal/platform/voice.c) is not there yet" >&2
  exit 1
fi
flags=(-std=c11 -O2 -g -isysroot "$sdk" -D_DARWIN_C_SOURCE -Wall -Wextra -Wno-unused-parameter
       -Wno-unused-function -Wno-unused-variable -Wno-missing-field-initializers -Wno-sign-compare)
# Generated Coax C is compiled with its own (quieter) flags.
gflags=(-std=c11 -O2 -g -isysroot "$sdk" -w)
objs=()
for c in "${engine[@]}"; do
  o="$out/$(basename "$c" .c).o"
  "$cc" "${gflags[@]}" -I "$gen" -c "$c" -o "$o"
  objs+=("$o")
done
"$cc" "${flags[@]}" -I "$gen" -I metareal/platform -c metareal/platform/voice.c -o "$out/voice.o"
objs+=("$out/voice.o")
for c in ws wav speech; do
  "$cc" "${flags[@]}" -I "$gen" -c "$here/$c.c" -o "$out/$c.o"
done
"$cc" "${flags[@]}" -I "$gen" -o "$out/voice-host" "$here/host.c" "$out/ws.o" "$out/wav.o" "$out/speech.o" "${objs[@]}" \
  -lcurl -lpthread -lm
"$cc" "${flags[@]}" -o "$out/voice-client" "$here/client.c" "$out/ws.o" "$out/wav.o" "${objs[@]}" -lm
echo "built $out/voice-host and $out/voice-client"
