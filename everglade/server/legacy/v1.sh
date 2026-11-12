#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: git worktree and the frozen tree's own build scripts)
# Everglade v1's room, frozen (docs/decisions/0005-production-room-runs-everglade-ii.md, #13267).
# Production rooms run Everglade II only; no production room runs v1. v1's room server, its
# offline room for the browser and its drift check build by name from V1_PIN below, the last
# commit whose room carried v1, until P1.3 (#13269) retires v1. Each runs in that commit's own
# tree (a detached worktree, kept under V1_TREE) with that tree's own compiler (its coax/build.sh),
# so nothing here changes when main moves on.
#   everglade/server/legacy/v1.sh tree          check out the frozen tree (once) and print its path
#   everglade/server/legacy/v1.sh room [OUT]    v1's room server (default TREE/coax/web/room/bin/room)
#   everglade/server/legacy/v1.sh worker [OUT]  its offline room, everglade-room.wasm (default
#                                               TREE/apps/openagents-web/build/everglade-room.wasm)
#   everglade/server/legacy/v1.sh drift [ARGS]  v1's room, then the Everglade drift check on the frozen
#                                               tree (headless Chrome; the frozen tree's pages:
#                                               coax/remote/pages.sh V1_PIN)
set -euo pipefail
V1_PIN=026878c1813bee71cbac0c0186a73ad9f53ec5bf   # main on 2026-11-12, before the cut
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
tree="${V1_TREE:-$HOME/.cache/everglade-v1/${V1_PIN:0:12}}"
checkout() {
  if [[ ! -e "$tree/.git" ]]; then
    mkdir -p "$(dirname "$tree")"
    git -C "$root" worktree add -q --detach "$tree" "$V1_PIN"
  fi
  [[ "$(git -C "$tree" rev-parse HEAD)" == "$V1_PIN" ]] || { echo "v1.sh: $tree is not at $V1_PIN" >&2; exit 2; }
  [[ -x "$tree/coax/bin/coax" ]] || (cd "$tree" && coax/build.sh >&2)
}
cmd="${1:-}"; shift || true
case "$cmd" in
  tree) checkout; echo "$tree" ;;
  room) checkout; out="${1:-$tree/coax/web/room/bin/room}"; [[ "$out" == /* ]] || out="$PWD/$out"; (cd "$tree" && coax/web/room/build.sh "$out") ;;
  worker) checkout; out="${1:-$tree/apps/openagents-web/build/everglade-room.wasm}"; [[ "$out" == /* ]] || out="$PWD/$out"; (cd "$tree" && coax/web/tool.sh coax/web/room/worker.coax "$out") ;;
  drift) checkout
    [[ -e "$tree/apps/openagents-web/build/everglade5e.wasm" ]] || (cd "$tree" && coax/remote/pages.sh "$V1_PIN")
    (cd "$tree" && coax/web/room/build.sh && coax/web/tool.sh everglade-drift "$@") ;;
  *) sed -n '9,15p' "$0" >&2; exit 2 ;;
esac
