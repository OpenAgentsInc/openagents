#!/usr/bin/env bash
# openagents.com's content tier on livehost (#12435): docs, legal text, the
# API docs, and the design-language stylesheet and script, deployed as the
# `openagents` tenant's manifest. A server whose environment names the
# pointer (OPENAGENTS_LIVE_RELAY, _STORE, _PUBKEY) reads them from the newest
# manifest within seconds, without a new revision; see
# docs/deployment/openagents-web.md, "Content deploys".
#
# Usage:
#   scripts/deploy/livehost.sh deploy     # build the content tier and point staging at it
#   scripts/deploy/livehost.sh rollback   # point staging back at the previous manifest
#   scripts/deploy/livehost.sh status     # what staging points at
#   scripts/deploy/livehost.sh promote HASH
#                     # point `live` at staging's manifest HASH, signed with
#                     # the separate live key; refused unless staging serves
#                     # exactly HASH. No production server follows `live` yet.
#
# Environment (defaults in brackets):
#   LIVEHOST_CHANNEL  [staging]; `live` is refused until promotion exists
#   LIVEHOST_STORE    [gs://openagentsgemini-livehost]
#   LIVEHOST_RELAY    [wss://relay.openagents.com]
#   LIVEHOST_KEY      [~/work/.secrets/livehost-openagents-staging.key]
#   LIVEHOST_LIVE_KEY [~/work/.secrets/livehost-openagents-live.key], promote only
#   CLOUDSDK_CONFIG   for the bucket [~/work/.secrets/gcloud-sa-config]
#   LIVEHOST_TOOLS_TARGET  where the deploy tools build
#                     [~/work/openagents-target-1-livehost-tools]. A target of
#                     their own: in the shared one, a content deploy waited
#                     minutes for other builds' artifact lock.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
channel="${LIVEHOST_CHANNEL:-staging}"
store="${LIVEHOST_STORE:-gs://openagentsgemini-livehost}"
relay="${LIVEHOST_RELAY:-wss://relay.openagents.com}"
key="${LIVEHOST_KEY:-$HOME/work/.secrets/livehost-openagents-staging.key}"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
export CARGO_TARGET_DIR="${LIVEHOST_TOOLS_TARGET:-$HOME/work/openagents-target-1-livehost-tools}"
unset CARGO_BUILD_BUILD_DIR
# Handler components build in their own target too.
export LIVEHOST_GUEST_TARGET="${LIVEHOST_GUEST_TARGET:-$HOME/work/openagents-target-1-livehost}"

if [[ "$channel" != staging ]]; then
  echo "livehost.sh: only the staging channel can be deployed from here" >&2
  exit 64
fi
[[ -f "$key" ]] || { echo "livehost.sh: no deploy key at $key" >&2; exit 66; }

livehost() {
  cargo run --quiet --locked --release --manifest-path "$root/Cargo.toml" -p livehost -- "$@"
}

# The Grid's build billboard (#12442): best-effort announcements, skipped
# when web.sh, which announces its own content deploys, called this script.
. "$root/scripts/deploy/deploy-event.sh"
announce=1
[[ -z "${DEPLOY_EVENT_RUN:-}" ]] || announce=0
export DEPLOY_EVENT_ROOT="$root"
content_event() {
  if [[ "$announce" == 1 ]]; then
    deploy_event staging "$1" content "$(git -C "$root" rev-parse HEAD 2> /dev/null || true)"
  fi
}

case "${1:-}" in
  deploy)
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    started=$(date +%s)
    [[ "$announce" == 0 ]] || deploy_event_begin
    content_event deploying
    if ! "$root/scripts/deploy/web-content.sh" "$work/content"; then
      content_event deploy_failed
      exit 1
    fi
    echo "livehost.sh: content assembled in $(( $(date +%s) - started )) s"
    if ! livehost deploy "$work/content" --tenant openagents --channel "$channel" \
      --store "$store" --relay "$relay" --key "$key"; then
      content_event deploy_failed
      exit 1
    fi
    content_event live
    ;;
  rollback)
    [[ "$announce" == 0 ]] || deploy_event_begin
    livehost rollback --tenant openagents --channel "$channel" \
      --store "$store" --relay "$relay" --key "$key"
    content_event rolled_back
    ;;
  promote)
    confirm="${2:?usage: livehost.sh promote MANIFEST_HASH}"
    live_key="${LIVEHOST_LIVE_KEY:-$HOME/work/.secrets/livehost-openagents-live.key}"
    [[ -f "$live_key" ]] || { echo "livehost.sh: no live key at $live_key" >&2; exit 66; }
    livehost promote --tenant openagents --channel live --from "$channel" \
      --from-pubkey "$(livehost pubkey --key "$key")" --confirm "$confirm" \
      --store "$store" --relay "$relay" --key "$live_key"
    ;;
  status)
    livehost status --tenant openagents --channel "$channel" \
      --store "$store" --relay "$relay" --key "$key"
    ;;
  *)
    echo "usage: livehost.sh deploy|rollback|status|promote HASH" >&2
    exit 64
    ;;
esac
