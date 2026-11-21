#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around the pinned tree's coax serve; holds no logic).
# gamebuilder.org on this Mac: the pinned tree's dev server (coax/web/serve.sh)
# on web/, with hot reload.
#   scripts/web.sh              http://127.0.0.1:8810/
#   scripts/web.sh check        the page's and the world's checks, the village's pack (the pinned one, admitted by the engine's reader,
#                               credited), and that the world's shaders are emitted from Coax
#   scripts/web.sh entrance     the first second, in a real browser (tools/entrance-check.coax; docs/web-world.md, "The first second"):
#                               the dev server on a port of its own, then headless Chrome with WebGPU times each game type from the
#                               press on its card to the first key taken and to a placed thing (2 seconds each at most), and looks
#                               for the page's words with WebGPU missing each way. More arguments go to the tool (--budget MS,
#                               --shots DIR). GB_ENTRANCE_URL: a dev server already running, used as it is.
set -euo pipefail
gb="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
export GB_ROOT="$gb" GB_VENDOR="$repo" COAX="${COAX:-$repo/coax/bin/coax}"
# The world's checks keep a whole Hud (the engine's bodies and their second of history) on the stack: 3.5 MB, three times over.
ulimit -s 65520 2>/dev/null || true
if [[ "${1:-}" == entrance ]]; then
  shift; cd "$GB_ROOT"; url="${GB_ENTRANCE_URL:-}"
  if [[ -z "$url" ]]; then
    listen="${GB_ENTRANCE_LISTEN:-127.0.0.1:8826}"; url="http://$listen/"; log="$(mktemp -t gb-entrance)"
    "$GB_VENDOR/coax/web/serve.sh" "$GB_ROOT/web" --listen "$listen" --build "$GB_ROOT/web/build.sh" >"$log" 2>&1 & server=$!
    trap 'kill "$server" 2>/dev/null || true; rm -f "$log"' EXIT
    # (the server says "engine rebuilt" once its first build is in: the page is whole from then)
    for _ in $(seq 1 600); do grep -q "engine rebuilt" "$log" && break; if grep -q "build failed" "$log" || ! kill -0 "$server" 2>/dev/null; then cat "$log" >&2; exit 1; fi; sleep 1; done
  fi
  "$GB_VENDOR/coax/web/tool.sh" tools/entrance-check.coax "$url" "$@"; exit $?
fi
if [[ "${1:-}" == check ]]; then "$COAX" test "$GB_ROOT/web/gpu-words.coax" --library; "$COAX" test "$GB_ROOT/tools/entrance-check.coax" --library; "$COAX" test "$GB_ROOT/web/site.coax" --library; "$COAX" test "$GB_ROOT/web/site-agents.coax" --library; "$COAX" test "$GB_ROOT/web/login.coax" --library; "$COAX" test "$GB_ROOT/web/chat.coax" --library; "$COAX" test "$GB_ROOT/web/props.coax" --library; "$COAX" test "$GB_ROOT/web/blocks.coax" --library; "$COAX" test "$GB_ROOT/web/level.coax" --library; "$COAX" test "$GB_ROOT/web/undo.coax" --library; "$COAX" test "$GB_ROOT/web/keep.coax" --library; "$COAX" test "$GB_ROOT/web/account.coax" --library; "$COAX" test "$GB_ROOT/web/publish.coax" --library; "$COAX" test "$GB_ROOT/web/hud.coax" --library; "$COAX" test "$GB_ROOT/web/gpu/hud.coax" --library; cd "$GB_ROOT"; "$GB_VENDOR/coax/web/tool.sh" tools/village-pack-check.coax web/assets/village.pack; exec "$COAX" wgsl web/gpu/hud.coax $(cat web/gpu/hud.module) -o /dev/null; fi
exec "$GB_VENDOR/coax/web/serve.sh" "$GB_ROOT/web" --listen "${GB_WEB_LISTEN:-127.0.0.1:8810}" --build "$GB_ROOT/web/build.sh" "$@"
