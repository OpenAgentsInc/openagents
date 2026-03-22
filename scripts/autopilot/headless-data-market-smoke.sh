#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMPDIR="$(mktemp -d)"
SMOKE_HOME="$TMPDIR/home"
RUNTIME_LOG="$TMPDIR/headless-data-market.log"
RELAY_LOG="$TMPDIR/relay.log"
MANIFEST_PATH="$TMPDIR/desktop-control.json"
IDENTITY_PATH="$SMOKE_HOME/.openagents/pylon/identity.mnemonic"
SETTINGS_PATH="$SMOKE_HOME/.openagents/autopilot-settings-v1.conf"

export CARGO_INCREMENTAL=0

find_free_port() {
  python3 - <<'PY'
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
PY
}

write_settings() {
  local path="$1"
  local relay_url="$2"
  local identity_path="$3"
  mkdir -p "$(dirname "$path")"
  cat >"$path" <<EOF
schema_version=2
primary_relay_url=${relay_url}
backup_relay_urls=
identity_path=${identity_path}
wallet_default_send_sats=1000
provider_max_queue_depth=1
reconnect_required=false
EOF
}

cleanup() {
  if [[ -n "${RUNTIME_PID:-}" ]] && kill -0 "$RUNTIME_PID" >/dev/null 2>&1; then
    kill "$RUNTIME_PID" >/dev/null 2>&1 || true
    wait "$RUNTIME_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${RELAY_PID:-}" ]] && kill -0 "$RELAY_PID" >/dev/null 2>&1; then
    kill "$RELAY_PID" >/dev/null 2>&1 || true
    wait "$RELAY_PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$TMPDIR" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "building headless smoke binaries"
(
  cd "$ROOT"
  cargo build \
    -p autopilot-desktop \
    --bin autopilot-headless-compute \
    --bin autopilot_headless_data_market \
    --bin autopilotctl
)

HEADLESS_COMPUTE_BIN="$ROOT/target/debug/autopilot-headless-compute"
HEADLESS_DATA_MARKET_BIN="$ROOT/target/debug/autopilot_headless_data_market"
AUTOPILOTCTL_BIN="$ROOT/target/debug/autopilotctl"

mkdir -p "$SMOKE_HOME/.openagents/pylon"
RELAY_PORT="$(find_free_port)"
RELAY_URL="ws://127.0.0.1:${RELAY_PORT}"

"$HEADLESS_COMPUTE_BIN" relay --listen "127.0.0.1:${RELAY_PORT}" >"$RELAY_LOG" 2>&1 &
RELAY_PID=$!

"$HEADLESS_COMPUTE_BIN" identity --identity-path "$IDENTITY_PATH" >"$TMPDIR/identity.json"
write_settings "$SETTINGS_PATH" "$RELAY_URL" "$IDENTITY_PATH"

(
  HOME="$SMOKE_HOME" \
    OPENAGENTS_DISABLE_CODEX=true \
    "$HEADLESS_DATA_MARKET_BIN" \
      --manifest-path "$MANIFEST_PATH"
) >"$RUNTIME_LOG" 2>&1 &
RUNTIME_PID=$!

echo "started autopilot-headless-data-market pid=$RUNTIME_PID"

STATUS_JSON="$TMPDIR/status.json"
for _ in $(seq 1 60); do
  if (
    "$AUTOPILOTCTL_BIN" \
      --manifest "$MANIFEST_PATH" \
      --json status
  ) >"$STATUS_JSON" 2>/dev/null; then
    break
  fi
  sleep 1
done

if [[ ! -s "$STATUS_JSON" ]]; then
  echo "headless data market runtime did not become reachable" >&2
  echo "runtime log:" >&2
  cat "$RUNTIME_LOG" >&2
  exit 1
fi

mkdir -p "$TMPDIR/source"
printf 'hello from headless data market smoke\n' >"$TMPDIR/source/sample.txt"

PACKAGE_DIR="$TMPDIR/package"
"$ROOT/scripts/autopilot/data_market_package.py" \
  --source "$TMPDIR/source" \
  --output-dir "$PACKAGE_DIR" \
  --title "Headless Smoke Bundle" \
  --default-policy targeted_request \
  --grant-policy-template targeted_request \
  --consumer-id npub1headlesssmoke \
  --grant-expires-hours 24 \
  --grant-warranty-window-hours 4 \
  --price-sats 250 \
  >"$TMPDIR/package-summary.json"

"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json data-market seller-status >"$TMPDIR/seller-status.json"

"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json data-market draft-asset \
  --file "$PACKAGE_DIR/listing-template.json" >"$TMPDIR/draft-asset.json"

"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json data-market preview-asset >"$TMPDIR/preview-asset.json"

"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json data-market publish-asset \
  --confirm >"$TMPDIR/publish-asset.json"

"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json data-market draft-grant \
  --file "$PACKAGE_DIR/grant-template.json" >"$TMPDIR/draft-grant.json"

"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json data-market preview-grant >"$TMPDIR/preview-grant.json"

"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json data-market publish-grant \
  --confirm >"$TMPDIR/publish-grant.json"

"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json data-market buyer-refresh >"$TMPDIR/buyer-refresh.json"
"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json data-market snapshot >"$TMPDIR/snapshot.json"

python3 - <<'PY' \
  "$TMPDIR/package-summary.json" \
  "$TMPDIR/draft-asset.json" \
  "$TMPDIR/preview-asset.json" \
  "$TMPDIR/publish-asset.json" \
  "$TMPDIR/draft-grant.json" \
  "$TMPDIR/preview-grant.json" \
  "$TMPDIR/publish-grant.json" \
  "$TMPDIR/buyer-refresh.json" \
  "$TMPDIR/snapshot.json"
import json
import sys

package_summary = json.load(open(sys.argv[1], encoding="utf-8"))
draft_asset = json.load(open(sys.argv[2], encoding="utf-8"))
preview_asset = json.load(open(sys.argv[3], encoding="utf-8"))
publish_asset = json.load(open(sys.argv[4], encoding="utf-8"))
draft_grant = json.load(open(sys.argv[5], encoding="utf-8"))
preview_grant = json.load(open(sys.argv[6], encoding="utf-8"))
publish_grant = json.load(open(sys.argv[7], encoding="utf-8"))
buyer_refresh = json.load(open(sys.argv[8], encoding="utf-8"))
snapshot = json.load(open(sys.argv[9], encoding="utf-8"))

seller_asset = publish_asset["payload"]["seller"]["published_assets"][0]
seller_grant = publish_grant["payload"]["seller"]["published_grants"][0]
buyer = buyer_refresh["payload"]["buyer"]
selected_listing = buyer["selected_listing"]
selected_offer = buyer["selected_catalog_offer"]

assert package_summary["content_digest"].startswith("sha256:")
assert draft_asset["payload"]["seller"]["draft"]["title"] == "Headless Smoke Bundle"
assert preview_asset["payload"]["seller"]["draft"]["last_previewed_asset_payload"] is not None
assert publish_asset["payload"]["seller"]["draft"]["last_published_asset_id"] is not None
assert seller_asset["ds_listing_coordinate"].startswith("30404:")
assert draft_grant["payload"]["seller"]["draft"]["grant_consumer_id"] == "npub1headlesssmoke"
assert preview_grant["payload"]["seller"]["draft"]["last_previewed_grant_payload"] is not None
assert publish_grant["payload"]["seller"]["draft"]["last_published_grant_id"] is not None
assert seller_grant["ds_offer_coordinate"].startswith("30406:")
assert selected_listing["coordinate"] == seller_asset["ds_listing_coordinate"]
assert selected_listing["linked_asset_id"] == seller_asset["asset_id"]
assert selected_offer["coordinate"] == seller_grant["ds_offer_coordinate"]
assert selected_offer["linked_grant_id"] == seller_grant["grant_id"]
assert snapshot["payload"]["market"]["relay_listing_count"] >= 1
assert snapshot["payload"]["market"]["relay_offer_count"] >= 1

print(json.dumps({
    "content_digest": package_summary["content_digest"],
    "published_asset_id": seller_asset["asset_id"],
    "published_grant_id": seller_grant["grant_id"],
    "ds_listing_coordinate": seller_asset["ds_listing_coordinate"],
    "ds_offer_coordinate": seller_grant["ds_offer_coordinate"],
    "selected_listing_coordinate": selected_listing["coordinate"],
    "selected_offer_coordinate": selected_offer["coordinate"],
}, indent=2, sort_keys=True))
PY
