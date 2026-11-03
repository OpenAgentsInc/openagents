#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles the Coax CLI to C and links it with the host C files)
# Builds the openagents CLI into apps/openagents-cli/build/openagents, and the
# dictation host `openagents dictate` runs beside it (build/openagents-dictate).
#   apps/openagents-cli/build.sh            then: apps/openagents-cli/build/openagents help
# Links sockets and TLS for the relay client (coax/platform/tls-env.sh:
# Network.framework on macOS; on Linux the system's OpenSSL 3, else OpenSSL from Nix).
# The dictation host links the shared dictation controller
# (apps/shared/dictation.coax, its own C unit) and Psionic's resident Parakeet
# (psionic/platform/parakeet-build.sh: Metal on macOS; on Linux CUDA, whose
# driver and NVRTC it opens at run time, or the CPU), plus
# CoreAudio and AVFoundation on macOS. It is its own program because the
# engine's generated C and the CLI's signing code export the same names.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../.." && pwd)"; p="$repo/coax/platform"
coax="${COAX:-$repo/coax/bin/coax}"
[[ -x "$coax" ]] || { echo "build the compiler first: coax/build.sh" >&2; exit 2; }
if [[ "$(uname -s)" == Darwin ]]; then
  cc=(/usr/bin/clang -D_DARWIN_C_SOURCE); libs=(-framework Security -framework CoreFoundation)
  dlibs=(-framework AVFoundation -framework AudioToolbox -framework CoreAudio -framework Metal -framework Foundation); dmac=("$here/dictate-mac.m" -fobjc-arc)
else cc=("${CC:-gcc}" -D_DEFAULT_SOURCE); libs=(-lpthread); dlibs=(-ldl); dmac=(); fi
. "$p/tls-env.sh"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/entropy-policy.h" > "$work/entropy.c"
"$coax" c "$here/main.coax" --library > "$work/openagents.c"   # main.coax declares its #run_steps
"$coax" c "$here/dictate-host.coax" --library > "$work/dictate-host.c"
"$coax" c "$repo/apps/shared/dictation.coax" --library --header "$work/dictation.h" > "$work/dictation.c"
COAX="$coax" COAX_CC="${cc[0]}" "$repo/psionic/platform/parakeet-build.sh" "$repo/psionic/build/parakeet" "$repo" > /dev/null
mkdir -p "$here/build"
# shellcheck disable=SC2086
"${cc[@]}" -std=c11 -O2 $CXTLS_CFLAGS -I"$work" -I"$p" -o "$work/openagents" "$work/openagents.c" "$here/cli-native.c" \
  "$p/tool-native.c" "$p/process-native.c" "$p/store-file-native.c" "$p/entropy-native.c" "$p/entropy-foreign.c" \
  "$p/socket-native.c" "$p/tls-native.c" "$work/entropy.c" "${libs[@]}" $CXTLS_LIBS -lm
"${cc[@]}" -std=c11 -O2 -I"$work" -I"$p" -I"$repo/psionic/platform" -o "$work/openagents-dictate" "$work/dictate-host.c" \
  "$here/cli-native.c" "$here/dictate-native.c" "$work/dictation.c" ${dmac[@]+"${dmac[@]}"} \
  "$p/tool-native.c" "$p/process-native.c" "$p/store-file-native.c" "$p/entropy-native.c" "$p/entropy-foreign.c" "$work/entropy.c" \
  "$repo/psionic/build/parakeet/libpsionic-parakeet.a" "${libs[@]}" ${dlibs[@]+"${dlibs[@]}"} -lpthread -lm
mv -f "$work/openagents" "$here/build/openagents"
mv -f "$work/openagents-dictate" "$here/build/openagents-dictate"
echo "$here/build/openagents"
