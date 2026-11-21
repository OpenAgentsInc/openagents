#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: it runs coax/web/tool.sh clip on the build view's link and holds no logic).
# Take SLAB's construction film again, headless: the build view (?view=build) from the empty yard to
# the commissioned platform, at 1920 x 1080 and 30 frames a second, with the HUD left out. Every
# frame is one fixed step of the page's clock (coax/web/clip.coax), so the same commit gives the
# same film however slowly the frames are captured.
#
#   scripts/slab-build-film.sh [OUT.mp4] [SERVER]
#
# OUT defaults to ~/.openagents/archive/docs/research/ocean/slab-construction.mp4. SERVER is a running dev server for this
# checkout (default http://127.0.0.1:9681); start one with
#   OA_SERVE_NO_API=1 OA_SERVE_NO_CODER=1 OA_SERVE_NO_ROOM=1 coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9681
# after building slab.wasm. The film's length is the film plan's (metaverse/slab-build-film.coax,
# about 215 s for the default run); FILM_SECONDS overrides it, FILM_SIZE the size.
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-$HOME/.openagents/archive/docs/research/ocean/slab-construction.mp4}"
server="${2:-http://127.0.0.1:9681}"
seconds="${FILM_SECONDS:-216}"
size="${FILM_SIZE:-1920x1080}"
exec coax/web/tool.sh clip "$server/worlds/slab?view=build&hud=0&style=photoreal" "$out" --size "$size" --seconds "$seconds" --fps 30
