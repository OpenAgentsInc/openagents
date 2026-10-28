#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
# Xcode exports an iOS SDKROOT; the bootstrap compiler runs on the build Mac.
# The explicit host triple also blocks inherited iOS deployment settings.
# Product output is ordinary C11, compiled by Xcode for the actual target SDK.
mkdir -p apps/openagents-ios/Generated
compiler=coax/bin/coax
if [[ ! -x "$compiler" || coax/build.sh -nt "$compiler" || apps/openagents-ios/generate.sh -nt "$compiler" ]] || [[ -n "$(find coax/src -type f -newer "$compiler" -print -quit)" ]]; then
  env -u CFLAGS -u CXXFLAGS SDKROOT="$(xcrun --sdk macosx --show-sdk-path)" \
    CXX="$(xcrun --sdk macosx --find clang++)" \
    COAX_HOST_TARGET="$(uname -m)-apple-macosx11.0" coax/build.sh
fi
out=apps/openagents-ios/Generated
"$compiler" c metareal/home.coax --library --header "$out/Home.h" > "$out/Home.c.tmp"
mv "$out/Home.c.tmp" "$out/Home.c"
"$compiler" metal metareal/light.coax > "$out/HomeShaders.txt.tmp"
cat apps/openagents-ios/OpenAgents/Platform/Stages.metal >> "$out/HomeShaders.txt.tmp"
mv "$out/HomeShaders.txt.tmp" "$out/HomeShaders.txt"
