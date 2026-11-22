#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around coax test and repo verification gates; holds no logic).
# Runs GameBuilder's full check suite: core, shared, standalone worlds, web shaders,
# tools, SPLICEBOTS port adapters, CLI end-to-end, layer-check, and tools-check.
set -euo pipefail
gb="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
export GB_ROOT="$gb" GB_VENDOR="$repo" COAX="${COAX:-$repo/coax/bin/coax}"
ulimit -s 65520 2>/dev/null || true

# Core, shared, and app integration
"$COAX" test "$GB_ROOT/core/controls.coax"
"$COAX" test "$GB_ROOT/core/app.coax" --only "the engine's page"
"$COAX" test "$GB_ROOT/shared/kinds.coax" --library
"$COAX" test "$GB_ROOT/shared/game-types.coax" --library

# Standalone worlds & web shaders
"$GB_ROOT/scripts/web.sh" check

# Tools, Unreal preview, SPLICEBOTS port adapters, engine-list, loop-check
"$COAX" test "$GB_ROOT/tools/unreal-preview.coax"
"$COAX" test "$repo/metaverse/splice-bots/gamebuilder-port/splicebots-port.coax"
"$COAX" test "$repo/metaverse/splice-bots/gamebuilder-port/splicebots-project.coax"
"$COAX" test "$GB_ROOT/tools/engine-list.coax" --only 'a chapter'
(cd "$repo" && "$repo/coax/web/tool.sh" gamebuilder/tools/engine-list.coax --check)
"$COAX" test "$GB_ROOT/tools/loop-check.coax"
(cd "$GB_ROOT" && "$repo/coax/web/tool.sh" tools/loop-check.coax shared/loop.coax scripts/web-deploy.sh web/*.coax web/*.js core/*.coax cli/*.coax tools/*.coax)
"$COAX" test "$GB_ROOT/tests/player/provenance.coax" --library

# Static landing & world pages sanity
for f in "$GB_ROOT/web/index.html" "$GB_ROOT/web/index.md" "$GB_ROOT/web/credits.html" "$GB_ROOT/worlds/fated-dragon/index.html" "$GB_ROOT/worlds/splicebots/index.html" "$GB_ROOT/worlds/whispering-spring/index.html"; do
  [[ -s "$f" ]] || { echo "FAIL missing $f" >&2; exit 1; }
done

# Asset lockfile hydrator tests & repo layer/tool gates
"$COAX" test "$repo/coax/web/assets-sync.coax" --library
(cd "$repo" && "$repo/coax/web/tool.sh" layer-check)
(cd "$repo" && "$repo/coax/web/tool.sh" tools-check)

# GameBuilder CLI end-to-end
"$GB_ROOT/cli/check.sh"
echo "gamebuilder check: ok"
