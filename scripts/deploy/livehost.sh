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
#
# Environment (defaults in brackets):
#   LIVEHOST_CHANNEL  [staging]; `live` is refused until promotion exists
#   LIVEHOST_STORE    [gs://openagentsgemini-livehost]
#   LIVEHOST_RELAY    [wss://relay.openagents.com]
#   LIVEHOST_KEY      [~/work/.secrets/livehost-openagents-staging.key]
#   CLOUDSDK_CONFIG   for the bucket [~/work/.secrets/gcloud-sa-config]

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
channel="${LIVEHOST_CHANNEL:-staging}"
store="${LIVEHOST_STORE:-gs://openagentsgemini-livehost}"
relay="${LIVEHOST_RELAY:-wss://relay.openagents.com}"
key="${LIVEHOST_KEY:-$HOME/work/.secrets/livehost-openagents-staging.key}"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"

if [[ "$channel" != staging ]]; then
  echo "livehost.sh: only the staging channel can be deployed from here" >&2
  exit 64
fi
[[ -f "$key" ]] || { echo "livehost.sh: no deploy key at $key" >&2; exit 66; }

livehost() {
  cargo run --quiet --locked --release --manifest-path "$root/Cargo.toml" -p livehost -- "$@"
}

case "${1:-}" in
  deploy)
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    started=$(date +%s)
    "$root/scripts/deploy/web-content.sh" "$work/content"
    echo "livehost.sh: content assembled in $(( $(date +%s) - started )) s"
    livehost deploy "$work/content" --tenant openagents --channel "$channel" \
      --store "$store" --relay "$relay" --key "$key"
    ;;
  rollback)
    livehost rollback --tenant openagents --channel "$channel" \
      --store "$store" --relay "$relay" --key "$key"
    ;;
  status)
    livehost status --tenant openagents --channel "$channel" \
      --store "$store" --relay "$relay" --key "$key"
    ;;
  *)
    echo "usage: livehost.sh deploy|rollback|status" >&2
    exit 64
    ;;
esac
