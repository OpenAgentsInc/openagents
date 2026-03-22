#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMPDIR="$(mktemp -d)"
SMOKE_HOME="$TMPDIR/home"
RUNTIME_LOG="$TMPDIR/headless-data-market.log"
NEXUS_LOG="$TMPDIR/nexus-control.log"
RELAY_LOG="$TMPDIR/relay.log"
MANIFEST_PATH="$TMPDIR/desktop-control.json"
IDENTITY_PATH="$SMOKE_HOME/.openagents/pylon/identity.mnemonic"
SETTINGS_PATH="$SMOKE_HOME/.openagents/autopilot-settings-v1.conf"

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

authority_get_json() {
  local base_url="$1"
  local access_token="$2"
  local route="$3"
  local output_path="$4"
  python3 - <<'PY' "$base_url" "$access_token" "$route" "$output_path"
import json
import sys
import urllib.parse
import urllib.request

base_url = sys.argv[1].rstrip("/")
access_token = sys.argv[2]
route = sys.argv[3]
output_path = sys.argv[4]
request = urllib.request.Request(
    base_url + route,
    headers={
        "authorization": f"Bearer {access_token}",
        "accept": "application/json",
    },
    method="GET",
)
with urllib.request.urlopen(request, timeout=10) as response:
    payload = json.loads(response.read().decode("utf-8"))
with open(output_path, "w", encoding="utf-8") as handle:
    json.dump(payload, handle)
PY
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
  if [[ -n "${NEXUS_PID:-}" ]] && kill -0 "$NEXUS_PID" >/dev/null 2>&1; then
    kill "$NEXUS_PID" >/dev/null 2>&1 || true
    wait "$NEXUS_PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$TMPDIR" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "building headless smoke binaries"
(
  cd "$ROOT"
  cargo build \
    -p nexus-control \
    -p autopilot-desktop \
    --bin autopilot-headless-compute \
    --bin autopilot_headless_data_market \
    --bin autopilotctl
)

HEADLESS_COMPUTE_BIN="$ROOT/target/debug/autopilot-headless-compute"
HEADLESS_DATA_MARKET_BIN="$ROOT/target/debug/autopilot_headless_data_market"
AUTOPILOTCTL_BIN="$ROOT/target/debug/autopilotctl"
NEXUS_CONTROL_BIN="$ROOT/target/debug/nexus-control"

mkdir -p "$SMOKE_HOME/.openagents/pylon"
RELAY_PORT="$(find_free_port)"
RELAY_URL="ws://127.0.0.1:${RELAY_PORT}"

"$HEADLESS_COMPUTE_BIN" relay --listen "127.0.0.1:${RELAY_PORT}" >"$RELAY_LOG" 2>&1 &
RELAY_PID=$!

"$HEADLESS_COMPUTE_BIN" identity --identity-path "$IDENTITY_PATH" >"$TMPDIR/identity.json"

write_settings "$SETTINGS_PATH" "$RELAY_URL" "$IDENTITY_PATH"

(
  NEXUS_CONTROL_LISTEN_ADDR=127.0.0.1:0 \
    NEXUS_CONTROL_KERNEL_STATE_PATH="$TMPDIR/kernel-state.json" \
    NEXUS_CONTROL_RECEIPT_LOG_PATH="$TMPDIR/receipt-log.jsonl" \
    "$NEXUS_CONTROL_BIN"
) >"$NEXUS_LOG" 2>&1 &
NEXUS_PID=$!

echo "started nexus-control pid=$NEXUS_PID"

NEXUS_BASE_URL=""
for _ in $(seq 1 60); do
  if grep -Eq 'nexus-control listening on 127\.0\.0\.1:[0-9]+' "$NEXUS_LOG"; then
    PORT="$(sed -n 's/.*nexus-control listening on 127\.0\.0\.1:\([0-9][0-9]*\).*/\1/p' "$NEXUS_LOG" | tail -n 1)"
    if [[ -n "$PORT" ]]; then
      NEXUS_BASE_URL="http://127.0.0.1:$PORT"
      break
    fi
  fi
  sleep 1
done

if [[ -z "$NEXUS_BASE_URL" ]]; then
  echo "local nexus-control did not become reachable" >&2
  echo "nexus log:" >&2
  cat "$NEXUS_LOG" >&2
  exit 1
fi

NEXUS_SESSION_JSON="$TMPDIR/nexus-session.json"
python3 - <<'PY' "$NEXUS_BASE_URL" "$NEXUS_SESSION_JSON"
import json
import sys
import urllib.request

base_url = sys.argv[1]
output_path = sys.argv[2]
payload = {
    "desktop_client_id": "autopilot-headless-data-market-smoke",
    "device_name": "Headless Data Market Smoke",
    "client_version": "smoke",
}
request = urllib.request.Request(
    base_url + "/api/session/desktop",
    data=json.dumps(payload).encode("utf-8"),
    headers={"content-type": "application/json"},
    method="POST",
)
with urllib.request.urlopen(request, timeout=5) as response:
    body = response.read().decode("utf-8")
parsed = json.loads(body)
with open(output_path, "w", encoding="utf-8") as handle:
    json.dump(parsed, handle)
PY

NEXUS_ACCESS_TOKEN="$(python3 - <<'PY' "$NEXUS_SESSION_JSON"
import json
import sys
with open(sys.argv[1], encoding="utf-8") as handle:
    payload = json.load(handle)
print(payload["access_token"])
PY
)"

(
  HOME="$SMOKE_HOME" \
    OPENAGENTS_DISABLE_CODEX=true \
    OA_CONTROL_BASE_URL="$NEXUS_BASE_URL" \
    OA_CONTROL_BEARER_TOKEN="$NEXUS_ACCESS_TOKEN" \
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
  echo "nexus log:" >&2
  cat "$NEXUS_LOG" >&2
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

"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json data-market snapshot >"$TMPDIR/snapshot-post-publish.json"

"$AUTOPILOTCTL_BIN" --manifest "$MANIFEST_PATH" --json data-market snapshot >"$TMPDIR/snapshot.json"

ASSET_ID="$(python3 - <<'PY' "$TMPDIR/publish-asset.json"
import json
import pathlib
import sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
print(payload["payload"]["seller"]["draft"]["last_published_asset_id"])
PY
)"
GRANT_ID="$(python3 - <<'PY' "$TMPDIR/publish-grant.json"
import json
import pathlib
import sys
payload = json.loads(pathlib.Path(sys.argv[1]).read_text())
print(payload["payload"]["seller"]["draft"]["last_published_grant_id"])
PY
)"

authority_get_json "$NEXUS_BASE_URL" "$NEXUS_ACCESS_TOKEN" \
  "/v1/kernel/data/assets/$(python3 - <<'PY' "$ASSET_ID"
import sys, urllib.parse
print(urllib.parse.quote(sys.argv[1], safe=""))
PY
)" \
  "$TMPDIR/authority-asset.json"
authority_get_json "$NEXUS_BASE_URL" "$NEXUS_ACCESS_TOKEN" \
  "/v1/kernel/data/grants/$(python3 - <<'PY' "$GRANT_ID"
import sys, urllib.parse
print(urllib.parse.quote(sys.argv[1], safe=""))
PY
)" \
  "$TMPDIR/authority-grant.json"

python3 - <<'PY' \
  "$TMPDIR/package-summary.json" \
  "$TMPDIR/draft-asset.json" \
  "$TMPDIR/preview-asset.json" \
  "$TMPDIR/publish-asset.json" \
  "$TMPDIR/draft-grant.json" \
  "$TMPDIR/preview-grant.json" \
  "$TMPDIR/publish-grant.json" \
  "$TMPDIR/snapshot-post-publish.json" \
  "$TMPDIR/snapshot.json" \
  "$TMPDIR/authority-asset.json" \
  "$TMPDIR/authority-grant.json"
import json
import sys

package_summary = json.load(open(sys.argv[1], encoding="utf-8"))
draft_asset = json.load(open(sys.argv[2], encoding="utf-8"))
preview_asset = json.load(open(sys.argv[3], encoding="utf-8"))
publish_asset = json.load(open(sys.argv[4], encoding="utf-8"))
draft_grant = json.load(open(sys.argv[5], encoding="utf-8"))
preview_grant = json.load(open(sys.argv[6], encoding="utf-8"))
publish_grant = json.load(open(sys.argv[7], encoding="utf-8"))
snapshot_post_publish = json.load(open(sys.argv[8], encoding="utf-8"))
snapshot = json.load(open(sys.argv[9], encoding="utf-8"))
authority_asset = json.load(open(sys.argv[10], encoding="utf-8"))
authority_grant = json.load(open(sys.argv[11], encoding="utf-8"))

assert package_summary["content_digest"].startswith("sha256:")
assert draft_asset["payload"]["seller"]["draft"]["title"] == "Headless Smoke Bundle"
assert preview_asset["payload"]["seller"]["draft"]["last_previewed_asset_payload"] is not None
assert publish_asset["payload"]["seller"]["draft"]["last_published_asset_id"] is not None
assert draft_grant["payload"]["seller"]["draft"]["grant_consumer_id"] == "npub1headlesssmoke"
assert preview_grant["payload"]["seller"]["draft"]["last_previewed_grant_payload"] is not None
assert publish_grant["payload"]["seller"]["draft"]["last_published_grant_id"] is not None
assert snapshot_post_publish["payload"]["seller"]["draft"]["last_published_asset_id"]
assert snapshot_post_publish["payload"]["seller"]["draft"]["last_published_grant_id"]
assert snapshot["payload"]["seller"]["draft"]["title"] == "Headless Smoke Bundle"
asset_nostr = authority_asset["asset"]["nostr_publications"]
grant_nostr = authority_grant["grant"]["nostr_publications"]
assert asset_nostr["ds_listing"]["coordinate"].startswith("30404:")
assert asset_nostr["ds_listing"]["event_id"]
assert grant_nostr["ds_offer"]["coordinate"].startswith("30406:")
assert grant_nostr["ds_offer"]["event_id"]

print(json.dumps({
    "content_digest": package_summary["content_digest"],
    "preview_ready": preview_asset["payload"]["seller"]["draft"]["preview_posture"],
    "published_asset_id": snapshot_post_publish["payload"]["seller"]["draft"]["last_published_asset_id"],
    "published_grant_id": snapshot_post_publish["payload"]["seller"]["draft"]["last_published_grant_id"],
    "ds_listing_coordinate": asset_nostr["ds_listing"]["coordinate"],
    "ds_offer_coordinate": grant_nostr["ds_offer"]["coordinate"],
    "snapshot_title": snapshot["payload"]["seller"]["draft"]["title"],
}, indent=2, sort_keys=True))
PY
