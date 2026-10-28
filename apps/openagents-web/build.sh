#!/usr/bin/env bash
# Builds the pages for the browser: build/splash.wasm and build/project.wasm.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/build"
# The HUD bar's light, the same function the iOS stages draw (metareal/light.coax).
(cd "$here/../.." && python3 coax/web/wgsl.py metareal/light.coax metareal_bar --into apps/openagents-web/splash.wgsl)
"$here/../../coax/web/wasm.sh" "$here/splash.coax" "$here/face.c" "$here/build/splash.wasm" &
splash=$!
"$here/../../coax/web/wasm.sh" "$here/project.coax" "$here/project-face.c" "$here/build/project.wasm"
wait "$splash"
