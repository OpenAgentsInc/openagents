#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax/web/wasm.sh, wgsl-module.coax, assets-sync and dragon-bake; holds no logic).
# Builds gamebuilder.org for the browser: static landing page (/ and /credits), shared Coaxial host
# (/coaxial/*.js + /build/{frame-plan,host-layers,host-packs}.wasm), and standalone worlds:
#   /fated-dragon      (gamebuilder/worlds/fated-dragon/)
#   /splicebots        (gamebuilder/worlds/splicebots/ + metaverse/splice-bots/)
#   /whispering-spring (gamebuilder/worlds/whispering-spring/)
#   /ice-cream-man     (gamebuilder/worlds/ice-cream-man/ + metaverse/icecream/)
#   /scrappers-arena   (gamebuilder/worlds/scrappers-arena/ + gamebuilder/core/play.coax)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
gb="$(cd "$here/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
export GB_ROOT="$gb" GB_VENDOR="$repo" COAX="${COAX:-$repo/coax/bin/coax}"
ulimit -s 65520 2>/dev/null || true

mkdir -p "$here/build" "$here/coaxial" "$here/credits" \
  "$here/fated-dragon/build" "$here/fated-dragon/assets" \
  "$here/splicebots/build" "$here/splicebots/fonts" "$here/splicebots/packs" \
  "$here/whispering-spring/build" \
  "$here/ice-cream-man/build" "$here/ice-cream-man/assets" \
  "$here/scrappers-arena/build" "$here/scrappers-arena/assets" \
  "$gb/build"

# Ensure stamp.coax exists for gamebuilder/core/commands.coax (used by Scrappers Arena)
if [[ ! -s "$gb/build/stamp.coax" ]]; then
  head_sha="$(git -C "$repo" rev-parse HEAD 2>/dev/null || echo "0000000000")"
  printf 'GB_COMMIT :: "%s";\nGB_DIRTY :: false;\n' "${head_sha:0:10}" > "$gb/build/stamp.coax"
fi

# Hydrate content-addressed binary packs (SPLICEBOTS packs + GameBuilder assets)
(cd "$repo" && COAX="$COAX" "$repo/coax/web/tool.sh" assets-sync --group gamebuilder)

# Static pages for local dev server (/credits -> credits/index.html)
cp "$here/credits.html" "$here/credits/index.html"

# Shared Coaxial host JS + helper Wasm modules (resolved via ../build/*.wasm from /coaxial/*.js)
cp "$repo/coax/web/"*.js "$here/coaxial/"
"$repo/coax/web/wasm.sh" "$repo/metaverse-engine/host/frame-plan-carrier.coax" "$repo/coax/web/frame-plan-face.c" "$here/build/frame-plan.wasm"
"$repo/coax/web/tool.sh" "$repo/coax/web/build-layers.coax" "$repo" "$here/build/host-layers.wasm"
"$repo/coax/web/tool.sh" "$repo/coax/web/build-packs.coax" "$repo" "$here/build/host-packs.wasm"

# World 1: Fated Dragon (/fated-dragon)
fd_src="$gb/worlds/fated-dragon"
cp "$fd_src/index.html" "$here/fated-dragon/index.html"
cp "$fd_src/glue.js" "$here/fated-dragon/glue.js"
dragon_gbd1="$here/fated-dragon/assets/dragon.gbd1"
if [[ ! -s "$dragon_gbd1" || "$repo/assets/metaverse/beasts/dragon.gltf" -nt "$dragon_gbd1" || "$repo/assets/metaverse/beasts/dragon.bin" -nt "$dragon_gbd1" || "$gb/tools/dragon-bake.coax" -nt "$dragon_gbd1" ]]; then
  (cd "$repo" && COAX="$COAX" "$repo/coax/web/tool.sh" gamebuilder/tools/dragon-bake.coax assets/metaverse/beasts/dragon.gltf "$dragon_gbd1")
fi
"$repo/coax/web/wasm.sh" "$fd_src/fated-dragon.coax" "$fd_src/fated-dragon-face.c" "$here/fated-dragon/build/fated-dragon.wasm"
(cd "$repo" && COAX="$COAX" COAXIAL_MODE="${COAXIAL_MODE:-release}" "$repo/coax/web/tool.sh" "$repo/coax/web/wgsl-module.coax" gamebuilder/worlds/fated-dragon/gpu/fated-dragon.coax "$here/fated-dragon/fated-dragon.wgsl" gamebuilder/worlds/fated-dragon/gpu/fated-dragon.module)

# World 2: SPLICEBOTS (/splicebots)
sb_src="$gb/worlds/splicebots"
cp "$sb_src/index.html" "$here/splicebots/index.html"
for f in IBMPlexMono-Medium.ttf IBMPlexMono-OFL.txt Michroma-Regular.ttf Michroma-OFL.txt; do
  cp "$repo/apps/openagents-web/realms/splice-bots/assets/fonts/$f" "$here/splicebots/fonts/$f"
done
"$repo/coax/web/wasm.sh" "$repo/metaverse/splice-bots/splice-jam.coax" "$repo/metaverse/splice-bots/splice-jam-face.c" "$here/splicebots/build/splice-jam.wasm" "$repo/apps/openagents-web/animelab-style.coax"
(cd "$repo" && COAX="$COAX" COAXIAL_MODE="${COAXIAL_MODE:-release}" "$repo/coax/web/tool.sh" "$repo/coax/web/wgsl-module.coax" metaverse/splice-bots/gpu/splice-jam.coax "$here/splicebots/splice-jam.wgsl" metaverse/splice-bots/gpu/splice-jam.module)

# World 3: The Whispering Spring (/whispering-spring)
ws_src="$gb/worlds/whispering-spring"
cp "$ws_src/index.html" "$ws_src/glue.js" "$ws_src/audio.js" "$ws_src/room.js" "$ws_src/audio-manifest.json" "$here/whispering-spring/"
# the modelled grove pieces (the Obsidian Quail grove asset kit), baked again when a model or the baker changes
mkdir -p "$here/whispering-spring/assets"
ws_models=("$ws_src/models/earthen-bank.glb" "$ws_src/models/cedar.glb" "$ws_src/models/limestone.glb" "$ws_src/models/exposed-roots.glb" "$ws_src/models/pool-and-spillway.glb")
ws_grove="$here/whispering-spring/assets/grove.wsg"
ws_stale=0; [[ -s "$ws_grove" && ! "$gb/tools/grove-bake.coax" -nt "$ws_grove" ]] || ws_stale=1
for m in "${ws_models[@]}"; do [[ "$m" -nt "$ws_grove" ]] && ws_stale=1; done
if [[ $ws_stale == 1 ]]; then
  (cd "$repo" && COAX="$COAX" "$repo/coax/web/tool.sh" gamebuilder/tools/grove-bake.coax "$ws_grove" "${ws_models[@]}")
fi
"$repo/coax/web/wasm.sh" "$ws_src/world-spring.coax" "$ws_src/whispering-spring-face.c" "$here/whispering-spring/build/whispering-spring.wasm"
(cd "$repo" && COAX="$COAX" COAXIAL_MODE="${COAXIAL_MODE:-release}" "$repo/coax/web/tool.sh" "$repo/coax/web/wgsl-module.coax" gamebuilder/worlds/whispering-spring/gpu/whispering-spring.coax "$here/whispering-spring/whispering-spring.wgsl" gamebuilder/worlds/whispering-spring/gpu/whispering-spring.module)

# World 4: Ice Cream Man (/ice-cream-man)
icm_src="$gb/worlds/ice-cream-man"
cp "$icm_src/index.html" "$here/ice-cream-man/index.html"
cp "$repo/apps/openagents-web/realms/ice-cream-man/assets/IceCream.indices" "$here/ice-cream-man/assets/IceCream.indices"
COAXIAL_RENDER=0 "$repo/coax/web/wasm.sh" "$repo/apps/openagents-web/icecream.coax" "$icm_src/icecream-face.c" "$here/ice-cream-man/build/icecream.wasm" "$repo/metaverse/icecream/scene.coax"
(cd "$repo" && COAX="$COAX" COAXIAL_MODE="${COAXIAL_MODE:-release}" "$repo/coax/web/tool.sh" "$repo/coax/web/wgsl-module.coax" apps/openagents-web/gpu/icecream.coax "$here/ice-cream-man/icecream.wgsl" apps/openagents-web/gpu/icecream.module)

# World 5: Scrappers Arena (/scrappers-arena)
sa_src="$gb/worlds/scrappers-arena"
cp "$sa_src/index.html" "$sa_src/glue.js" "$here/scrappers-arena/"
cp "$gb/assets/scrappers.gbm" "$here/scrappers-arena/assets/scrappers.gbm"
COAXIAL_RUN_STEPS=100000000 "$repo/coax/web/wasm.sh" "$sa_src/scrappers.coax" "$sa_src/scrappers-face.c" "$here/scrappers-arena/build/scrappers-arena.wasm"
(cd "$repo" && COAX="$COAX" COAXIAL_MODE="${COAXIAL_MODE:-release}" "$repo/coax/web/tool.sh" "$repo/coax/web/wgsl-module.coax" gamebuilder/worlds/scrappers-arena/gpu/scrappers-arena.coax "$here/scrappers-arena/scrappers-arena.wgsl" gamebuilder/worlds/scrappers-arena/gpu/scrappers-arena.module)
