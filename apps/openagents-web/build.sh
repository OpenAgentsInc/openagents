#!/usr/bin/env bash
# Builds the pages for the browser: build/splash.wasm and build/project.wasm.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/build"
# The HUD bar's light, the same function the iOS stages draw (metareal/light.coax).
(cd "$here/../.." && python3 coax/web/wgsl.py metareal/light.coax metareal_bar --into apps/openagents-web/splash.wgsl)
"$here/../../coax/web/wasm.sh" "$here/splash.coax" "$here/face.c" "$here/build/splash.wasm" &
splash=$!
# The project page's tasks, read from Nostr into project/coax/work.coax: a
# release always reads afresh, a dev build when the snapshot is ten minutes old.
# (Unreachable relay: the last snapshot stays.)
tools="$here/../../coax/web/project"
[[ -d "$tools/node_modules" ]] || (cd "$tools" && bun install --silent)
fresh=(); [[ "${COAXIAL_MODE:-dev}" == release ]] || fresh=(--if-older 10)
bun "$tools/snapshot.ts" --project coax ${fresh[@]+"${fresh[@]}"}
"$here/../../coax/web/wasm.sh" "$here/project.coax" "$here/project-face.c" "$here/build/project.wasm"
wait "$splash"
