#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
# Xcode exports an iOS SDKROOT; the bootstrap compiler runs on the build Mac.
# Product output is ordinary C11, compiled by Xcode for the actual target SDK.
mkdir -p apps/openagents-ios/Generated
compiler=coax/bin/coax
if [[ ! -x "$compiler" ]] || [[ -n "$(find coax/src -type f -newer "$compiler" -print -quit)" ]]; then
  env -u SDKROOT -u CFLAGS -u CXXFLAGS CXX="$(xcrun --sdk macosx --find clang++)" coax/build.sh
fi
out=apps/openagents-ios/Generated
"$compiler" c metareal/home.coax --library --header "$out/Home.h" > "$out/Home.c.tmp"
mv "$out/Home.c.tmp" "$out/Home.c"
"$compiler" metal metareal/light.coax > "$out/HomeShaders.txt.tmp"
cat apps/openagents-ios/OpenAgents/Platform/Stages.metal >> "$out/HomeShaders.txt.tmp"
mv "$out/HomeShaders.txt.tmp" "$out/HomeShaders.txt"
