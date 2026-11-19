#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: it runs coax/web/tool.sh clip on one SLAB link and holds no logic).
# Take SLAB's flythrough film again, headless: the finished platform from far out over the sea, in
# low past its south-east corner, along the north wall with the skyline over it, up past the power
# station's stacks, over the city, and out to the whole platform from high in the south-east, at
# 1920 x 1080 and 30 frames a second with the HUD left out. The camera flies the page's ?fly= path
# (apps/openagents-web/slab-tour.coax); every frame is one fixed step of the page's clock
# (coax/web/clip.coax), so the same commit gives the same film.
#
#   scripts/slab-flythrough.sh [OUT.mp4] [SERVER]
#
# SERVER is a running dev server for this checkout (default http://127.0.0.1:9681); start one with
#   OA_SERVE_NO_API=1 OA_SERVE_NO_CODER=1 OA_SERVE_NO_ROOM=1 coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9681
# after building slab.wasm. FILM_SIZE overrides the size. A still of any second of the path is
#   coax/web/tool.sh shot "SERVER/worlds/slab?hud=0&tour=0&t=40&fly=PATH&flyat=SECONDS" OUT.png 1600x900 6000
set -euo pipefail
cd "$(dirname "$0")/.."
out="${1:-slab-flythrough.mp4}"
server="${2:-http://127.0.0.1:9681}"
size="${FILM_SIZE:-1920x1080}"
# seconds, the eye, the point looked at (metres: +X east along the length, +Y up from the calm
# waterline, +Z north; the deck is at 118 m)
path='0,2400,60,1500,0,90,0~8,1500,90,950,0,100,0~16,800,40,520,0,90,-80~24,500,70,720,100,170,250~32,-300,90,730,-250,170,200~40,-760,170,360,-300,140,0~48,-380,400,40,0,200,0~56,280,380,-280,-50,200,50~64,800,450,-600,0,120,0~72,1100,700,900,100,100,60~80,1000,650,1050,120,100,80'
# two seconds on the last view
exec coax/web/tool.sh clip "$server/worlds/slab?hud=0&tour=0&fly=$path" "$out" --size "$size" --seconds 82 --fps 30 --fresh
