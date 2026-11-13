#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: compiles the Coax CLI to C and links it with the host C files)
# Builds the openagents CLI into apps/openagents-cli/build/openagents, and the
# dictation host `openagents dictate` runs beside it (build/openagents-dictate).
#   apps/openagents-cli/build.sh            then: apps/openagents-cli/build/openagents help
#   apps/openagents-cli/build.sh --cli      only build/openagents, not the dictation host (the web build,
#                                           apps/openagents-web/build.sh, runs only its project verbs)
# --cli keeps the built CLI in the tools' cache (COAX_TOOL_CACHE, default ~/.cache/coax-tools;
# 0: none) under what it is made from: the compiler's bytes, every file main.coax loads, the
# native C, and the C compiler with its flags. A fresh tree with the same CLI sources (each
# release builds in one) takes it in a second instead of compiling it again (#13154).
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
only_cli=false; [[ "${1:-}" != --cli ]] || only_cli=true
coax="${COAX:-$repo/coax/bin/coax}"
[[ -x "$coax" ]] || { echo "build the compiler first: coax/build.sh" >&2; exit 2; }
if [[ "$(uname -s)" == Darwin ]]; then
  cc=(/usr/bin/clang -D_DARWIN_C_SOURCE); libs=(-framework Security -framework CoreFoundation)
  dlibs=(-framework AVFoundation -framework AudioToolbox -framework CoreAudio -framework Metal -framework Foundation); dmac=("$here/dictate-mac.m" -fobjc-arc)
else cc=("${CC:-gcc}" -D_DEFAULT_SOURCE); libs=(-lpthread); dlibs=(-ldl); dmac=(); fi
. "$p/tls-env.sh"
(cd "$repo" && COAX="$coax" "$repo/coax/web/tool.sh" metaverse-engine/tools/agent-help-build.coax --check) > /dev/null
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
cli_units=("$here/cli-native.c" "$p/tool-native.c" "$p/process-native.c" "$p/store-file-native.c" "$p/entropy-native.c" "$p/entropy-foreign.c" "$p/socket-native.c" "$p/tls-native.c")
if $only_cli; then
  cache="${COAX_TOOL_CACHE:-$HOME/.cache/coax-tools}"; key=""
  if [[ "$cache" != 0 ]] && loads="$(cd "$repo" && { COAX="$coax" coax/web/tool.sh wasm-size sources apps/openagents-cli/main.coax && COAX="$coax" coax/web/tool.sh wasm-size sources coax/lib/crypto/entropy.coax; })" && [[ -n "$loads" ]]; then
    # shellcheck disable=SC2086
    key="$({ echo openagents-cli-1; shasum -a 256 < "$coax"; (cd "$repo" && shasum -a 256 $loads) || echo unreadable "$RANDOM$RANDOM"
             shasum -a 256 "$here/cli-native.c" "$p"/*.c "$p"/*.h "$p"/*.inc "$p/tls-env.sh" | sed "s|  $repo/|  |"
             "${cc[0]}" --version; uname -sm; printf '%s\n' "${cc[@]}" "${libs[@]}" "$CXTLS_CFLAGS" "$CXTLS_LIBS"; } 2> /dev/null | shasum -a 256 | cut -c1-64)"
  fi
  mkdir -p "$here/build"
  if [[ -n "$key" && -x "$cache/cli-$key" ]] && cp "$cache/cli-$key" "$work/openagents" 2> /dev/null; then
    touch "$cache/cli-$key" 2> /dev/null || true
  else
    "$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/entropy-policy.h" > "$work/entropy.c"
    # By its path in the repository, so the kept CLI's messages name no one checkout.
    (cd "$repo" && "$coax" c apps/openagents-cli/main.coax --library) > "$work/openagents.c"   # main.coax declares its #run_steps
    # shellcheck disable=SC2086
    "${cc[@]}" -std=c11 -O2 $CXTLS_CFLAGS -I"$work" -I"$p" -o "$work/openagents" "$work/openagents.c" "${cli_units[@]}" "$work/entropy.c" "${libs[@]}" $CXTLS_LIBS -lm
    if [[ -n "$key" ]] && mkdir -p "$cache" 2> /dev/null && cp "$work/openagents" "$cache/cli-$key.$$" 2> /dev/null; then
      mv -f "$cache/cli-$key.$$" "$cache/cli-$key" 2> /dev/null || rm -f "$cache/cli-$key.$$"
    fi
  fi
  mv -f "$work/openagents" "$here/build/openagents"
  echo "$here/build/openagents"
  exit 0
fi
"$coax" c "$repo/coax/lib/crypto/entropy.coax" --library --header "$work/entropy-policy.h" > "$work/entropy.c"
"$coax" c "$here/main.coax" --library > "$work/openagents.c"   # main.coax declares its #run_steps
"$coax" c "$here/dictate-host.coax" --library > "$work/dictate-host.c"
"$coax" c "$repo/apps/shared/dictation.coax" --library --header "$work/dictation.h" > "$work/dictation.c"
COAX="$coax" COAX_CC="${cc[0]}" "$repo/psionic/platform/parakeet-build.sh" "$repo/psionic/build/parakeet" "$repo" > /dev/null
mkdir -p "$here/build"
# shellcheck disable=SC2086
"${cc[@]}" -std=c11 -O2 $CXTLS_CFLAGS -I"$work" -I"$p" -o "$work/openagents" "$work/openagents.c" "${cli_units[@]}" "$work/entropy.c" "${libs[@]}" $CXTLS_LIBS -lm
"${cc[@]}" -std=c11 -O2 -I"$work" -I"$p" -I"$repo/psionic/platform" -o "$work/openagents-dictate" "$work/dictate-host.c" \
  "$here/cli-native.c" "$here/dictate-native.c" "$work/dictation.c" ${dmac[@]+"${dmac[@]}"} \
  "$p/tool-native.c" "$p/process-native.c" "$p/store-file-native.c" "$p/entropy-native.c" "$p/entropy-foreign.c" "$work/entropy.c" \
  "$repo/psionic/build/parakeet/libpsionic-parakeet.a" "${libs[@]}" ${dlibs[@]+"${dlibs[@]}"} -lpthread -lm
mv -f "$work/openagents" "$here/build/openagents"
mv -f "$work/openagents-dictate" "$here/build/openagents-dictate"
if [[ "$(uname -s)" == Linux ]]; then
  (cd "$repo" && COAX="$coax" coax/web/tool.sh apps/openagents-cli/dictate-stream-build.coax) > /dev/null
fi
echo "$here/build/openagents"
