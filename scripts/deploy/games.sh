#!/usr/bin/env bash
# The website's game tenants on livehost (#12486): The Haunted House (`haunt`)
# and The Ice Cream Man (`icecream`), served at openagents.com/t/<tenant>/ by
# servers whose environment names them (OPENAGENTS_LIVE_GAMES, see
# docs/deployment/openagents-web.md, "Game tenants"). Each tenant signs its
# own pointer with its own key; this script never touches the `openagents`
# content tenant.
#
# Usage:
#   scripts/deploy/games.sh pubkey GAME          the tenant key's public key, for OPENAGENTS_LIVE_GAMES
#   scripts/deploy/games.sh deploy GAME          build the game and point its staging channel at it
#   scripts/deploy/games.sh deploy GAME --live   the same, on the live channel production follows
#   scripts/deploy/games.sh rollback GAME [--live]
#
# GAME is `haunt` or `icecream`.
#
# Environment (defaults in brackets):
#   LIVEHOST_STORE   [gs://openagentsgemini-livehost], the content tier's store
#   LIVEHOST_RELAY   [wss://relay.openagents.com]
#   GAME_KEY         [~/work/.secrets/livehost-GAME.key], the tenant's deploy key
#   GAMES_WORK       [~/.openagents/games-live], the build and bake folder
#   CLOUDSDK_CONFIG  for the bucket [~/work/.secrets/gcloud-sa-config]
#   GAMES_TOOLS_TARGET  where the tools build [~/work/openagents-target-1-livehost-tools]
#   GAMES_WASM_TARGET   where the games' browser builds go [~/work/openagents-target-1-icecream-wasm]

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
command="${1:-}"
game="${2:-}"
case "$game" in
  haunt | icecream) ;;
  *) echo "usage: games.sh pubkey|deploy|rollback haunt|icecream [--live]" >&2; exit 64 ;;
esac
channel=staging
[[ "${3:-}" == --live ]] && channel=live
store="${LIVEHOST_STORE:-gs://openagentsgemini-livehost}"
relay="${LIVEHOST_RELAY:-wss://relay.openagents.com}"
key="${GAME_KEY:-$HOME/work/.secrets/livehost-$game.key}"
work="${GAMES_WORK:-$HOME/.openagents/games-live}"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
tools_target="${GAMES_TOOLS_TARGET:-$HOME/work/openagents-target-1-livehost-tools}"
wasm_target="${GAMES_WASM_TARGET:-$HOME/work/openagents-target-1-icecream-wasm}"

[[ -f "$key" ]] || { echo "games.sh: no deploy key for $game at $key" >&2; exit 66; }

# Each Cargo run uses its target's source-keyed build state (AGENTS.md).
in_target() {
  (
    export CARGO_TARGET_DIR="$1"
    unset RUSTC CARGO_BUILD_BUILD_DIR
    . "$root/scripts/dev/cargo-build-env.sh" >/dev/null
    "${@:2}"
  )
}

tool() {
  in_target "$tools_target" cargo run --quiet --locked --release \
    --manifest-path "$root/Cargo.toml" -p "$1" -- "${@:2}"
}

case "$command" in
  pubkey)
    tool livehost pubkey --key "$key"
    ;;
  deploy)
    mkdir -p "$work"
    if [[ "$game" == haunt ]]; then
      in_target "$wasm_target" "$root/scripts/build-haunt-web.sh" "$work/haunt-shell"
    else
      in_target "$wasm_target" "$root/scripts/build-icecream-web.sh" "$work/shell"
    fi
    tool icecream-live deploy --game "$game" --channel "$channel" --work "$work" \
      --store "$store" --relay "$relay" --key "$key" --host https://openagents.com
    ;;
  rollback)
    tool icecream-live rollback --game "$game" --channel "$channel" --work "$work" \
      --store "$store" --relay "$relay" --key "$key" --host https://openagents.com
    ;;
  *)
    echo "usage: games.sh pubkey|deploy|rollback haunt|icecream [--live]" >&2
    exit 64
    ;;
esac
