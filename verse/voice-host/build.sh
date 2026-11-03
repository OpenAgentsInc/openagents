#!/usr/bin/env bash
# Builds the voice host and the dev client into verse/voice-host/build/.
#   verse/voice-host/build.sh
# The Coax engine (verse/voice*.coax) becomes one C library per file with
# coax/bin/coax, exactly as apps/openagents-ios/generate.sh does for the app;
# verse/platform/voice.c is the C face over it. The client also links
# Aethyr's spoken-request gate (aethyr/voice.coax). Every host links Psionic's
# resident Parakeet (libpsionic-parakeet.a from psionic/platform/parakeet-build.sh;
# Metal on macOS, the CPU elsewhere). On macOS libcurl comes from the SDK and
# the psionic provider links Psionic's Whisper (whisper-build.sh) and Metal;
# Whisper does not build on Linux yet, so a Linux host is built without it
# (OA_VOICE_NO_WHISPER) and takes libcurl and OpenSSL from the system, or
# from Nix when the system has none (NixOS).
# #authority files run their checks inside `coax c`, so a failing
# check stops the build.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
cd "$root"
compiler=coax/bin/coax
out="$here/build"
gen="$out/gen"
mkdir -p "$gen"
if [[ "$(uname)" == Darwin ]]; then
  sdk="$(xcrun --sdk macosx --show-sdk-path)"
  cc="$(xcrun --sdk macosx --find clang)"
  if [[ ! -x "$compiler" ]] || [[ -n "$(find coax/src coax/lib coax/bootstrap -type f -newer "$compiler" -print -quit)" ]]; then
    env -u CFLAGS -u CXXFLAGS SDKROOT="$sdk" CXX="$(xcrun --sdk macosx --find clang++)" \
      COAX_HOST_TARGET="$(uname -m)-apple-macosx11.0" coax/build.sh
  fi
  sysflags=(-isysroot "$sdk" -D_DARWIN_C_SOURCE)
  hostlibs=(-framework Metal -framework Foundation -lcurl)
  defs=()
else
  cc="${CC:-cc}"
  [[ -x "$compiler" ]] || { echo "build.sh: no Coax compiler at $compiler (coax/build.sh)" >&2; exit 1; }
  sysflags=(-D_GNU_SOURCE)
  netinc=() netlib=()
  if ! pkg-config --exists libcurl openssl 2> /dev/null; then
    # NixOS: libcurl and OpenSSL from Nix (the OpenSSL exception in AGENTS.md).
    paths=($(nix-build '<nixpkgs>' -A curl.dev -A openssl.dev -A curl.out -A openssl.out --no-out-link))
    netinc=(-I "${paths[0]}/include" -I "${paths[1]}/include")
    netlib=(-L "${paths[2]}/lib" -L "${paths[3]}/lib" -Wl,-rpath,"${paths[2]}/lib" -Wl,-rpath,"${paths[3]}/lib")
  fi
  sysflags+=(${netinc[@]+"${netinc[@]}"})
  hostlibs=(${netlib[@]+"${netlib[@]}"} -lcurl -lcrypto)
  defs=(-DOA_VOICE_NO_WHISPER)
fi
engine=()
shopt -s nullglob
# The engine, plus the host's own reply-path decisions (voice-host/voice-reply.coax).
for src in verse/voice*.coax verse/voice-host/*.coax; do
  name="$(basename "$src" .coax)"                                    # voice-wire
  base="$(printf '%s' "$name" | awk -F- '{for(i=1;i<=NF;i++) printf toupper(substr($i,1,1)) substr($i,2)}')"  # VoiceWire
  # voice.coax -> VoiceEngine.h: "Voice.h" would be voice.h on a case-insensitive disk.
  [[ "$base" == "Voice" ]] && base=VoiceEngine
  "$compiler" c "$src" --library --header "$gen/$base.h" > "$gen/$base.c.tmp.$$"
  mv "$gen/$base.c.tmp.$$" "$gen/$base.c"
  engine+=("$gen/$base.c")
done
if [[ ${#engine[@]} -eq 0 || ! -f verse/platform/voice.c ]]; then
  echo "build.sh: the voice engine (verse/voice*.coax, verse/platform/voice.c) is not there yet" >&2
  exit 1
fi
"$compiler" c aethyr/voice.coax --library --header "$gen/AethyrVoice.h" > "$gen/AethyrVoice.c.tmp.$$"
mv "$gen/AethyrVoice.c.tmp.$$" "$gen/AethyrVoice.c"
flags=(-std=c11 -O2 -g "${sysflags[@]}" -Wall -Wextra -Wno-unused-parameter
       -Wno-unused-function -Wno-unused-variable -Wno-missing-field-initializers -Wno-sign-compare)
# Generated Coax C is compiled with its own (quieter) flags.
gflags=(-std=c11 -O2 -g "${sysflags[@]}" -w)
objs=()
for c in "${engine[@]}"; do
  o="$out/$(basename "$c" .c).o"
  "$cc" "${gflags[@]}" -I "$gen" -c "$c" -o "$o"
  objs+=("$o")
done
"$cc" "${gflags[@]}" -I "$gen" -c "$gen/AethyrVoice.c" -o "$out/AethyrVoice.o"
"$cc" "${flags[@]}" -I "$gen" -I verse/platform -c verse/platform/voice.c -o "$out/voice.o"
objs+=("$out/voice.o")
parakeet="$out/parakeet"
if [[ "$(uname)" == Darwin ]]; then
  SDKROOT="$sdk" COAX_CC="$cc" psionic/platform/parakeet-build.sh "$parakeet" > /dev/null
  psionic="$out/psionic"
  SDKROOT="$sdk" COAX_CC="$cc" psionic/platform/whisper-build.sh "$psionic" > /dev/null
  speechlibs=("$parakeet/libpsionic-parakeet.a" "$psionic/libpsionic-whisper.a")
else
  COAX_CC="$cc" psionic/platform/parakeet-build.sh "$parakeet" > /dev/null
  speechlibs=("$parakeet/libpsionic-parakeet.a")
fi
for c in ws wav speech; do
  "$cc" "${flags[@]}" ${defs[@]+"${defs[@]}"} -I "$gen" -I psionic/platform -c "$here/$c.c" -o "$out/$c.o"
done
"$cc" "${flags[@]}" ${defs[@]+"${defs[@]}"} -I "$gen" -o "$out/voice-host" "$here/host.c" "$out/ws.o" "$out/wav.o" "$out/speech.o" "${objs[@]}" \
  "${speechlibs[@]}" "${hostlibs[@]}" -lpthread -lm
"$cc" "${flags[@]}" -I "$gen" -o "$out/voice-client" "$here/client.c" "$out/ws.o" "$out/wav.o" "$out/AethyrVoice.o" "${objs[@]}" -lm
echo "built $out/voice-host and $out/voice-client"
