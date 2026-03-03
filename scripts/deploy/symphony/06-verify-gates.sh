#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud
require_cmd jq

ensure_gcloud_context

if ! instance_exists "$SYMPHONY_VM"; then
  die "VM does not exist: ${SYMPHONY_VM}"
fi

REPORT_DIR="${REPORT_DIR:-$ROOT_DIR/docs/reports/symphony}"
mkdir -p "$REPORT_DIR"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RECEIPT_PATH="${REPORT_DIR}/${STAMP}-deploy-receipt.json"

INSTANCE_STATUS="$(gcloud compute instances describe "$SYMPHONY_VM" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --format='value(status)')"
INSTANCE_ID="$(gcloud compute instances describe "$SYMPHONY_VM" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --format='value(id)')"

if [[ "$INSTANCE_STATUS" != "RUNNING" ]]; then
  die "Instance status is ${INSTANCE_STATUS}, expected RUNNING"
fi

TMP_REMOTE_SCRIPT="$(mktemp)"
trap 'rm -f "$TMP_REMOTE_SCRIPT"' EXIT

cat >"$TMP_REMOTE_SCRIPT" <<'REMOTE'
#!/usr/bin/env bash
set -euo pipefail

CFG="/etc/symphony/mainnet.toml"
RPC_USER="$(grep '^rpc_user' "$CFG" | cut -d'"' -f2)"
RPC_PASS="$(grep '^rpc_pass' "$CFG" | cut -d'"' -f2)"
RPC_ADDR="$(grep '^rpc_address' "$CFG" | cut -d'"' -f2)"

TIP_JSON="$(curl -fsS http://127.0.0.1:8080/tip)"
SYMPHONY_HEIGHT="$(printf '%s' "$TIP_JSON" | jq -r '.height // .block_height // .data.block_height // empty')"

RPC_PAYLOAD='{"jsonrpc":"1.0","id":"symphony","method":"getblockchaininfo","params":[]}'
BITCOIND_JSON="$(curl -fsS --user "${RPC_USER}:${RPC_PASS}" \
  --data-binary "$RPC_PAYLOAD" \
  -H 'content-type: text/plain;' \
  "$RPC_ADDR")"
BITCOIND_HEIGHT="$(printf '%s' "$BITCOIND_JSON" | jq -r '.result.blocks')"

printf '{"symphony_height":%s,"bitcoind_height":%s}\n' "$SYMPHONY_HEIGHT" "$BITCOIND_HEIGHT"
REMOTE

chmod +x "$TMP_REMOTE_SCRIPT"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_SCRIPT" "${SYMPHONY_VM}:/tmp/symphony-verify-probe.sh" >/dev/null

SERVICE_STATUS_RAW="$(gcloud compute ssh "$SYMPHONY_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "systemctl is-active symphony || true")"
SERVICE_STATUS="$(printf '%s' "$SERVICE_STATUS_RAW" | tail -n1 | tr -d '\r')"

PROBE_RAW="$(gcloud compute ssh "$SYMPHONY_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "chmod +x /tmp/symphony-verify-probe.sh && sudo /tmp/symphony-verify-probe.sh" || true)"
PROBE_JSON="$(printf '%s' "$PROBE_RAW" | tail -n1)"

SYMPHONY_HEIGHT="$(printf '%s' "$PROBE_JSON" | jq -r '.symphony_height // empty' 2>/dev/null || true)"
BITCOIND_HEIGHT="$(printf '%s' "$PROBE_JSON" | jq -r '.bitcoind_height // empty' 2>/dev/null || true)"

TIP_LAG=-1
if [[ -n "$SYMPHONY_HEIGHT" && -n "$BITCOIND_HEIGHT" ]]; then
  TIP_LAG=$((BITCOIND_HEIGHT - SYMPHONY_HEIGHT))
  if (( TIP_LAG < 0 )); then
    TIP_LAG=0
  fi
fi

SAMPLE_ADDR="${SAMPLE_ADDR:-bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh}"
ADDR_QUERY_RAW="$(gcloud compute ssh "$SYMPHONY_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "curl -fsS http://127.0.0.1:8080/addresses/${SAMPLE_ADDR}/tx_count" || true)"
ADDR_QUERY="$(printf '%s' "$ADDR_QUERY_RAW" | tail -n1)"
ADDR_OK=false
if printf '%s' "$ADDR_QUERY" | jq -e '.data != null' >/dev/null 2>&1; then
  ADDR_OK=true
fi

ERROR_LOG_COUNT="$(gcloud logging read \
  "resource.type=\"gce_instance\" AND resource.labels.instance_id=\"${INSTANCE_ID}\" AND severity>=ERROR" \
  --project "$GCP_PROJECT" \
  --freshness=30m \
  --limit=200 \
  --format='value(timestamp)' | wc -l | tr -d ' ')"

TIP_OK=false
if [[ -n "$SYMPHONY_HEIGHT" ]]; then
  TIP_OK=true
fi

SERVICE_OK=false
if [[ "$SERVICE_STATUS" == "active" ]]; then
  SERVICE_OK=true
fi

ERROR_LOG_SCAN_OK=false
if [[ "$ERROR_LOG_COUNT" -lt 10 ]]; then
  ERROR_LOG_SCAN_OK=true
fi

cat >"$RECEIPT_PATH" <<JSON
{
  "timestamp_utc": "${STAMP}",
  "project": "${GCP_PROJECT}",
  "region": "${GCP_REGION}",
  "zone": "${GCP_ZONE}",
  "environment": "prod",
  "vm": "${SYMPHONY_VM}",
  "service_status": "${SERVICE_STATUS}",
  "health_checks": {
    "service_active": ${SERVICE_OK},
    "tip_ok": ${TIP_OK},
    "api_sample_ok": ${ADDR_OK},
    "error_log_scan_ok": ${ERROR_LOG_SCAN_OK}
  },
  "tip": {
    "symphony_height": ${SYMPHONY_HEIGHT:-null},
    "bitcoind_height": ${BITCOIND_HEIGHT:-null},
    "lag_blocks": ${TIP_LAG}
  },
  "sample_address": "${SAMPLE_ADDR}",
  "error_log_count_last_30m": ${ERROR_LOG_COUNT}
}
JSON

log "Wrote deploy receipt: ${RECEIPT_PATH}"
cat "$RECEIPT_PATH"
