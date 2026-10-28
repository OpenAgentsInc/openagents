#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
# Xcode exports an iOS SDKROOT; the bootstrap compiler runs on the build Mac.
# The explicit host triple also blocks inherited iOS deployment settings.
# Product output is ordinary C11, compiled by Xcode for the actual target SDK.
mkdir -p apps/openagents-ios/Generated
compiler=coax/bin/coax
if [[ ! -x "$compiler" || coax/build.sh -nt "$compiler" || apps/openagents-ios/generate.sh -nt "$compiler" ]] || [[ -n "$(find coax/src coax/lib -type f -newer "$compiler" -print -quit)" ]]; then
  env -u CFLAGS -u CXXFLAGS SDKROOT="$(xcrun --sdk macosx --show-sdk-path)" \
    CXX="$(xcrun --sdk macosx --find clang++)" \
    COAX_HOST_TARGET="$(uname -m)-apple-macosx11.0" coax/build.sh
fi
out=apps/openagents-ios/Generated
for name in home player grid; do
  # Home, Player, Grid: one C library and header each.
  base="$(printf '%s' "${name:0:1}" | tr a-z A-Z)${name:1}"
  "$compiler" c "metareal/$name.coax" --library --header "$out/$base.h" > "$out/$base.c.tmp"
  mv "$out/$base.c.tmp" "$out/$base.c"
done
"$compiler" metal metareal/light.coax > "$out/HomeShaders.txt.tmp"
cat apps/openagents-ios/OpenAgents/Platform/Stages.metal >> "$out/HomeShaders.txt.tmp"
mv "$out/HomeShaders.txt.tmp" "$out/HomeShaders.txt"
