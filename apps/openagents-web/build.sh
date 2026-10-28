#!/usr/bin/env bash
# Builds the pages for the browser: build/splash.wasm, build/project.wasm and
# build/realm.wasm.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$here/build"
# The HUD bar's light, the same function the iOS stages draw (verse/light.coax).
(cd "$here/../.." && python3 coax/web/wgsl.py verse/light.coax verse_bar --into apps/openagents-web/splash.wgsl)
"$here/../../coax/web/wasm.sh" "$here/splash.coax" "$here/face.c" "$here/build/splash.wasm" &
splash=$!
# The realms: the engine's house, player and thumbs, each its own library as on
# iOS, under realm.coax's HUD; the house's light from the engine itself.
(cd "$here/../.." && python3 coax/web/wgsl.py verse/light.coax verse_bar verse_tone --into apps/openagents-web/realm.wgsl \
  && python3 coax/web/wgsl.py verse/haunt-light.coax verse_haunt_ambient verse_haunt_attenuate verse_haunt_shade --into apps/openagents-web/realm.wgsl)
m="$here/../../verse"
"$here/../../coax/web/wasm.sh" "$here/realm.coax" "$here/realm-face.c" "$here/build/realm.wasm" \
  "$m/haunt.coax" "$m/haunt-buffer.coax" "$m/player.coax" "$m/grid.coax" &
realm=$!
# The project page's tasks, read from Nostr into project/coax/work.coax: a
# release always reads afresh, a dev build when the snapshot is ten minutes old.
# (Unreachable relay: the last snapshot stays.)
tools="$here/../../coax/web/project"
[[ -d "$tools/node_modules" ]] || (cd "$tools" && bun install --silent)
fresh=(); [[ "${COAXIAL_MODE:-dev}" == release ]] || fresh=(--if-older 10)
bun "$tools/snapshot.ts" --project coax ${fresh[@]+"${fresh[@]}"} &
coax=$!
bun "$tools/snapshot.ts" --project verse ${fresh[@]+"${fresh[@]}"}
wait "$coax"
"$here/../../coax/web/wasm.sh" "$here/project.coax" "$here/project-face.c" "$here/build/project.wasm"
wait "$splash"
wait "$realm"
