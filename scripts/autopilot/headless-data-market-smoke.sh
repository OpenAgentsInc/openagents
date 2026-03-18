#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMPDIR="$(mktemp -d)"
RUNTIME_LOG="$TMPDIR/headless-data-market.log"
NEXUS_LOG="$TMPDIR/nexus-control.log"
MANIFEST_PATH="$TMPDIR/desktop-control.json"

cleanup() {
  if [[ -n "${RUNTIME_PID:-}" ]] && kill -0 "$RUNTIME_PID" >/dev/null 2>&1; then
    kill "$RUNTIME_PID" >/dev/null 2>&1 || true
    wait "$RUNTIME_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${NEXUS_PID:-}" ]] && kill -0 "$NEXUS_PID" >/dev/null 2>&1; then
    kill "$NEXUS_PID" >/dev/null 2>&1 || true
    wait "$NEXUS_PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$TMPDIR"
}
trap cleanup EXIT

(
  cd "$ROOT"
  NEXUS_CONTROL_LISTEN_ADDR=127.0.0.1:0 \
    NEXUS_CONTROL_KERNEL_STATE_PATH="$TMPDIR/kernel-state.json" \
    NEXUS_CONTROL_RECEIPT_LOG_PATH="$TMPDIR/receipt-log.jsonl" \
    cargo run -p nexus-control
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
  cd "$ROOT"
  OA_CONTROL_BASE_URL="$NEXUS_BASE_URL" \
    OA_CONTROL_BEARER_TOKEN="$NEXUS_ACCESS_TOKEN" \
    cargo run -p autopilot-desktop --bin autopilot_headless_data_market -- \
      --manifest-path "$MANIFEST_PATH"
) >"$RUNTIME_LOG" 2>&1 &
RUNTIME_PID=$!

echo "started autopilot-headless-data-market pid=$RUNTIME_PID"

STATUS_JSON="$TMPDIR/status.json"
for _ in $(seq 1 60); do
  if (
    cd "$ROOT" &&
      cargo run -p autopilot-desktop --bin autopilotctl -- \
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

(
  cd "$ROOT"
  cargo run -p autopilot-desktop --bin autopilotctl -- \
    --manifest "$MANIFEST_PATH" \
    --json data-market seller-status
) >"$TMPDIR/seller-status.json"

(
  cd "$ROOT"
  cargo run -p autopilot-desktop --bin autopilotctl -- \
    --manifest "$MANIFEST_PATH" \
    --json data-market draft-asset \
    --file "$PACKAGE_DIR/listing-template.json"
) >"$TMPDIR/draft-asset.json"

(
  cd "$ROOT"
  cargo run -p autopilot-desktop --bin autopilotctl -- \
    --manifest "$MANIFEST_PATH" \
    --json data-market preview-asset
) >"$TMPDIR/preview-asset.json"

(
  cd "$ROOT"
  cargo run -p autopilot-desktop --bin autopilotctl -- \
    --manifest "$MANIFEST_PATH" \
    --json data-market publish-asset \
    --confirm
) >"$TMPDIR/publish-asset.json"

(
  cd "$ROOT"
  cargo run -p autopilot-desktop --bin autopilotctl -- \
    --manifest "$MANIFEST_PATH" \
    --json data-market draft-grant \
    --file "$PACKAGE_DIR/grant-template.json"
) >"$TMPDIR/draft-grant.json"

(
  cd "$ROOT"
  cargo run -p autopilot-desktop --bin autopilotctl -- \
    --manifest "$MANIFEST_PATH" \
    --json data-market preview-grant
) >"$TMPDIR/preview-grant.json"

(
  cd "$ROOT"
  cargo run -p autopilot-desktop --bin autopilotctl -- \
    --manifest "$MANIFEST_PATH" \
    --json data-market publish-grant \
    --confirm
) >"$TMPDIR/publish-grant.json"

(
  cd "$ROOT"
  cargo run -p autopilot-desktop --bin autopilotctl -- \
    --manifest "$MANIFEST_PATH" \
    --json data-market snapshot
) >"$TMPDIR/snapshot-post-publish.json"

(
  cd "$ROOT"
  cargo run -p autopilot-desktop --bin autopilotctl -- \
    --manifest "$MANIFEST_PATH" \
    --json data-market snapshot
) >"$TMPDIR/snapshot.json"

python3 - <<'PY' \
  "$TMPDIR/package-summary.json" \
  "$TMPDIR/draft-asset.json" \
  "$TMPDIR/preview-asset.json" \
  "$TMPDIR/publish-asset.json" \
  "$TMPDIR/draft-grant.json" \
  "$TMPDIR/preview-grant.json" \
  "$TMPDIR/publish-grant.json" \
  "$TMPDIR/snapshot-post-publish.json" \
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
snapshot_post_publish = json.load(open(sys.argv[8], encoding="utf-8"))
snapshot = json.load(open(sys.argv[9], encoding="utf-8"))

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

print(json.dumps({
    "content_digest": package_summary["content_digest"],
    "preview_ready": preview_asset["payload"]["seller"]["draft"]["preview_posture"],
    "published_asset_id": snapshot_post_publish["payload"]["seller"]["draft"]["last_published_asset_id"],
    "published_grant_id": snapshot_post_publish["payload"]["seller"]["draft"]["last_published_grant_id"],
    "snapshot_title": snapshot["payload"]["seller"]["draft"]["title"],
}, indent=2, sort_keys=True))
PY
