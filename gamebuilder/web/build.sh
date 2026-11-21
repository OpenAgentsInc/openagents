#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around the pinned tree's coax/web/wasm.sh; holds no logic).
# Builds gamebuilder.org for the browser: web/build/site.wasm (the page), web/build/site-agents.wasm (the page with /agents),
# web/build/hud.wasm (the GPU HUD under it) and web/host/app.js (the page's one script). The dev
# server (scripts/web.sh) runs this first and again when a source it has no
# list for changes.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
gb="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
export GB_ROOT="$gb" GB_VENDOR="$repo" COAX="${COAX:-$repo/coax/bin/coax}"
# The world's checks keep a whole Hud (the engine's bodies and their second of history) on the stack: 3.5 MB, three times over.
ulimit -s 65520 2>/dev/null || true
mkdir -p "$here/build"
COAX="$COAX" "$repo/coax/web/tool.sh" assets-sync --group gamebuilder
cp "$GB_ROOT/tests/player/permission-fixture.html" "$here/build/player-permission.html"
cp "$GB_ROOT/tests/player/focus-fixture.html" "$here/build/player-focus.html"
"$GB_VENDOR/coax/web/wasm.sh" "$here/site.coax" "$here/site-face.c" "$here/build/site.wasm"   # every page but /agents (web/page.coax, /agents left out)
"$GB_VENDOR/coax/web/wasm.sh" "$here/site-agents.coax" "$here/site-agents-face.c" "$here/build/site-agents.wasm"   # /agents' own (web/keep.js), the page with it
# The icons /agents draws (web/agents.coax, through the component kit: the tool line's marks, the select's chevrons,
# the drawer's button, copy): the pinned tree's, from Apps SDK UI (MIT; its NOTICE beside them, and a row on /credits),
# asked for by name at /build/icons/ only when that page shows one. The home page asks for none.
mkdir -p "$here/build/icons"
for icon in sidebar chevron-up chevron-down check x circle-dashed circle copy; do cp "$GB_VENDOR/components/icons/apps-sdk/$icon.svg" "$here/build/icons/"; done
cp "$GB_VENDOR/components/icons/NOTICE" "$here/build/icons/NOTICE"
# /login, /login/cli, /play, /credits and /agents are the same page (one module draws every address; /agents' has the chat with your agents in it). The dev server finds a page by its
# folder, so the page is copied there; the deployed site serves it under the names `login` and `play`.
for page in login login/cli play credits agents; do mkdir -p "$here/$page"; cp "$here/index.html" "$here/$page/index.html"; done
# The world and the HUD under the page: the GPU layer (hud.coax, an engine scene drawn by
# coax/web/host.js), and its shaders, emitted from Coax (gpu/hud.coax: the engine's stylized
# shading and ghost, and the HUD's strokes) into build/hud.wgsl. No WGSL is written by hand.
"$GB_VENDOR/coax/web/wasm.sh" "$here/hud.coax" "$here/hud-face.c" "$here/build/hud.wasm"
(cd "$repo" && COAX="$COAX" COAXIAL_MODE=release "$GB_VENDOR/coax/web/tool.sh" "$GB_VENDOR/coax/web/wgsl-module.coax" gamebuilder/web/gpu/hud.coax gamebuilder/web/build/hud.wgsl gamebuilder/web/gpu/hud.module)
# The page's one script (web/app.js): the pinned tree's page host, its GPU host, the glue and all
# they import, as one file (issue 12: a first visit asks for one script, not forty). The hosts'
# workers and worklets are asked for by name beside it, so the pinned tree's files are copied there too.
mkdir -p "$here/host"
cp "$GB_VENDOR/coax/web/"*.js "$here/host/"
(cd "$here" && bun build app.js --target browser --format esm --minify-whitespace --minify-syntax --outfile host/app.js > /dev/null)
# What the host itself runs: the engine's frame plan (which pass draws when) and its layer codec.
"$GB_VENDOR/coax/web/wasm.sh" "$GB_VENDOR/metaverse-engine/host/frame-plan-carrier.coax" "$GB_VENDOR/coax/web/frame-plan-face.c" "$here/build/frame-plan.wasm"
"$GB_VENDOR/coax/web/tool.sh" "$GB_VENDOR/coax/web/build-layers.coax" "$GB_VENDOR" "$here/build/host-layers.wasm"
# The engine's pack reader, which the host opens the village's pack with (web/assets/village.pack; coax/web/pack-host.js).
"$GB_VENDOR/coax/web/tool.sh" "$GB_VENDOR/coax/web/build-packs.coax" "$GB_VENDOR" "$here/build/host-packs.wasm"
"$GB_VENDOR/coax/web/tool.sh" "$GB_ROOT/tools/player-test.coax" --write-catalog "$GB_ROOT"
