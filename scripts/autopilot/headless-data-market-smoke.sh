#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMPDIR="$(mktemp -d)"
RUNTIME_LOG="$TMPDIR/headless-data-market.log"
MANIFEST_PATH="$TMPDIR/desktop-control.json"

cleanup() {
  if [[ -n "${RUNTIME_PID:-}" ]] && kill -0 "$RUNTIME_PID" >/dev/null 2>&1; then
    kill "$RUNTIME_PID" >/dev/null 2>&1 || true
    wait "$RUNTIME_PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$TMPDIR"
}
trap cleanup EXIT

(
  cd "$ROOT"
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
  exit 1
fi

mkdir -p "$TMPDIR/source"
printf 'hello from headless data market smoke\n' >"$TMPDIR/source/sample.txt"

PACKAGE_DIR="$TMPDIR/package"
"$ROOT/scripts/autopilot/data_market_package.py" \
  --source "$TMPDIR/source" \
  --output-dir "$PACKAGE_DIR" \
  --title "Headless Smoke Bundle" \
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
    --json data-market snapshot
) >"$TMPDIR/snapshot.json"

python3 - <<'PY' \
  "$TMPDIR/package-summary.json" \
  "$TMPDIR/draft-asset.json" \
  "$TMPDIR/preview-asset.json" \
  "$TMPDIR/snapshot.json"
import json
import sys

package_summary = json.load(open(sys.argv[1], encoding="utf-8"))
draft_asset = json.load(open(sys.argv[2], encoding="utf-8"))
preview_asset = json.load(open(sys.argv[3], encoding="utf-8"))
snapshot = json.load(open(sys.argv[4], encoding="utf-8"))

assert package_summary["content_digest"].startswith("sha256:")
assert draft_asset["payload"]["seller"]["draft"]["title"] == "Headless Smoke Bundle"
assert preview_asset["payload"]["seller"]["draft"]["last_previewed_asset_payload"] is not None
assert snapshot["payload"]["seller"]["draft"]["title"] == "Headless Smoke Bundle"

print(json.dumps({
    "content_digest": package_summary["content_digest"],
    "preview_ready": preview_asset["payload"]["seller"]["draft"]["preview_posture"],
    "snapshot_title": snapshot["payload"]["seller"]["draft"]["title"],
}, indent=2, sort_keys=True))
PY
