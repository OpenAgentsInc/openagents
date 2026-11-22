#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax/web/serve.sh and coax test; holds no logic).
# gamebuilder.org on this Mac: dev server on web/ (landing page, /fated-dragon, /splicebots, /whispering-spring, /ice-cream-man, /scrappers-arena, /credits).
#   gamebuilder/scripts/web.sh           http://127.0.0.1:8810/
#   gamebuilder/scripts/web.sh check     world tests, legacy shared HUD/site tests, shader emission checks
set -euo pipefail
gb="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
export GB_ROOT="$gb" GB_VENDOR="$repo" COAX="${COAX:-$repo/coax/bin/coax}"
ulimit -s 65520 2>/dev/null || true

if [[ "${1:-}" == check ]]; then
  mkdir -p "$gb/build"
  if [[ ! -s "$gb/build/stamp.coax" ]]; then
    head_sha="$(git -C "$repo" rev-parse HEAD 2>/dev/null || echo "0000000000")"
    printf 'GB_COMMIT :: "%s";\nGB_DIRTY :: false;\n' "${head_sha:0:10}" > "$gb/build/stamp.coax"
  fi
  "$COAX" test "$GB_ROOT/worlds/fated-dragon/mountain.coax" --library
  "$COAX" test "$GB_ROOT/worlds/fated-dragon/dragon-rig.coax" --library
  "$COAX" test "$GB_ROOT/worlds/fated-dragon/fated-dragon.coax" --library
  "$COAX" test "$GB_ROOT/tools/dragon-bake.coax"
  "$COAX" test "$GB_ROOT/worlds/whispering-spring/world-spring.coax" --library
  "$COAX" test "$GB_ROOT/tools/grove-bake.coax"
  "$COAX" test "$GB_ROOT/worlds/scrappers-arena/scrappers.coax" --library --only "scrappers arena boots"
  "$COAX" test "$GB_ROOT/web/ghost.coax" --library
  "$COAX" test "$GB_ROOT/web/hud.coax" --library
  "$COAX" test "$GB_ROOT/web/gpu/hud.coax" --library
  "$COAX" test "$GB_ROOT/web/site.coax" --library
  "$COAX" test "$GB_ROOT/web/site-agents.coax" --library
  cd "$repo"
  "$COAX" wgsl gamebuilder/worlds/fated-dragon/gpu/fated-dragon.coax $(cat gamebuilder/worlds/fated-dragon/gpu/fated-dragon.module) -o /dev/null
  "$COAX" wgsl gamebuilder/worlds/whispering-spring/gpu/whispering-spring.coax $(cat gamebuilder/worlds/whispering-spring/gpu/whispering-spring.module) -o /dev/null
  "$COAX" wgsl metaverse/splice-bots/gpu/splice-jam.coax $(cat metaverse/splice-bots/gpu/splice-jam.module) -o /dev/null
  "$COAX" wgsl apps/openagents-web/gpu/icecream.coax $(cat apps/openagents-web/gpu/icecream.module) -o /dev/null
  "$COAX" wgsl gamebuilder/worlds/scrappers-arena/gpu/scrappers-arena.coax $(cat gamebuilder/worlds/scrappers-arena/gpu/scrappers-arena.module) -o /dev/null
  exec "$COAX" wgsl gamebuilder/web/gpu/hud.coax $(cat gamebuilder/web/gpu/hud.module) -o /dev/null
fi
exec "$repo/coax/web/serve.sh" "$GB_ROOT/web" --listen "${GB_WEB_LISTEN:-127.0.0.1:8810}" --build "$GB_ROOT/web/build.sh" "$@"
