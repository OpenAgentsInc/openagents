#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud
require_cmd jq

ensure_gcloud_context

DEPLOY_IMAGE="${DEPLOY_IMAGE:-${NEXUS_IMAGE}}"
REPORT_DIR="${ROOT_DIR}/docs/reports/nexus"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
RECEIPT_PATH="${REPORT_DIR}/${STAMP}-deploy-receipt.json"
VERIFY_HEALTH_LATENCY_MAX_MS="${VERIFY_HEALTH_LATENCY_MAX_MS:-1000}"
VERIFY_STATS_LATENCY_MAX_MS="${VERIFY_STATS_LATENCY_MAX_MS:-1000}"
VERIFY_TREASURY_LATENCY_MAX_MS="${VERIFY_TREASURY_LATENCY_MAX_MS:-1000}"
VERIFY_TREASURY_SNAPSHOT_MAX_AGE_MS="${VERIFY_TREASURY_SNAPSHOT_MAX_AGE_MS:-15000}"
VERIFY_TREASURY_WALLET_SYNC_MAX_LAG_MS="${VERIFY_TREASURY_WALLET_SYNC_MAX_LAG_MS:-15000}"

mkdir -p "$REPORT_DIR"

if ! instance_exists "$NEXUS_VM"; then
  die "VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

ssh_vm() {
  local remote_command="$1"
  gcloud compute ssh "$NEXUS_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "$remote_command"
}

fetch_json_probe() {
  local url="$1"
  ssh_vm "$(cat <<EOF
python3 - <<'PY'
import json
import time
import urllib.request

url = '${url}'
started = time.time()
with urllib.request.urlopen(url, timeout=20) as response:
    body = response.read().decode()

print(json.dumps({
    "body": json.loads(body),
    "latency_ms": int((time.time() - started) * 1000),
}))
PY
EOF
)"
}

fetch_treasury_env_json() {
  ssh_vm "$(cat <<'EOF'
python3 - <<'PY'
import json
from pathlib import Path

path = Path('/etc/nexus-relay/nexus-relay.env')
if not path.exists():
    print('null')
    raise SystemExit(0)

values = {}
for raw_line in path.read_text().splitlines():
    line = raw_line.strip()
    if not line or line.startswith('#') or '=' not in line:
        continue
    key, value = line.split('=', 1)
    values[key] = value

def parse_bool(key: str):
    return values.get(key, '').strip().lower() == 'true'

def parse_u64(key: str):
    raw = values.get(key, '').strip()
    return int(raw) if raw else None

payload = {
    "treasury_enabled": parse_bool("NEXUS_CONTROL_TREASURY_ENABLED"),
    "payout_sats_per_window": parse_u64("NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW"),
    "payout_interval_seconds": parse_u64("NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS"),
    "require_sellable": parse_bool("NEXUS_CONTROL_TREASURY_REQUIRE_SELLABLE"),
    "daily_budget_cap_sats": parse_u64("NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS"),
}
print(json.dumps(payload))
PY
EOF
)"
}

INSTANCE_STATUS="$(gcloud compute instances describe "$NEXUS_VM" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --format='value(status)')"

HEALTH_RESULT="$(fetch_json_probe "http://127.0.0.1:8080/healthz")"
STATS_RESULT="$(fetch_json_probe "http://127.0.0.1:8080/api/stats")"

if [[ "${NEXUS_CONTROL_TREASURY_ENABLED}" == "true" ]]; then
  TREASURY_RESULT="$(fetch_json_probe "http://127.0.0.1:8080/v1/treasury/status")"
  TREASURY_ENV_JSON="$(fetch_treasury_env_json)"
else
  TREASURY_RESULT='{"body":null,"latency_ms":0}'
  TREASURY_ENV_JSON='null'
fi

SERVICE_STATUS_RAW="$(ssh_vm "systemctl is-active nexus-relay")"
DATA_DIR_STATUS_RAW="$(ssh_vm "mount | grep '${NEXUS_DATA_DIR}' || true")"

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg vm "$NEXUS_VM" \
  --arg instance_status "$INSTANCE_STATUS" \
  --arg service_status "${SERVICE_STATUS_RAW//$'\n'/}" \
  --arg image "$DEPLOY_IMAGE" \
  --arg data_mount "$DATA_DIR_STATUS_RAW" \
  --argjson health_result "$HEALTH_RESULT" \
  --argjson stats_result "$STATS_RESULT" \
  --argjson treasury_result "$TREASURY_RESULT" \
  --argjson treasury_env "$TREASURY_ENV_JSON" \
  --argjson verify_health_latency_max_ms "$VERIFY_HEALTH_LATENCY_MAX_MS" \
  --argjson verify_stats_latency_max_ms "$VERIFY_STATS_LATENCY_MAX_MS" \
  --argjson verify_treasury_latency_max_ms "$VERIFY_TREASURY_LATENCY_MAX_MS" \
  --argjson verify_treasury_snapshot_max_age_ms "$VERIFY_TREASURY_SNAPSHOT_MAX_AGE_MS" \
  --argjson verify_treasury_wallet_sync_max_lag_ms "$VERIFY_TREASURY_WALLET_SYNC_MAX_LAG_MS" \
  '
  def gate($id; $passed; $reason; $observed):
    {
      gate_id: $id,
      passed: $passed,
      reason: $reason,
      observed: $observed
    };
  def treasury_policy_status:
    if ($treasury_result.body == null) then null else {
      treasury_enabled: $treasury_result.body.treasury_enabled,
      payout_sats_per_window: $treasury_result.body.payout_sats_per_window,
      payout_interval_seconds: $treasury_result.body.payout_interval_seconds,
      require_sellable: $treasury_result.body.require_sellable,
      daily_budget_cap_sats: $treasury_result.body.daily_budget_cap_sats,
      policy_checksum: $treasury_result.body.policy_checksum,
      policy_runtime_status: $treasury_result.body.policy_runtime_status
    } end;
  def policy_drift_fields:
    if ($treasury_result.body == null or $treasury_env == null) then []
    else [
      if $treasury_result.body.treasury_enabled != $treasury_env.treasury_enabled then "treasury_enabled" else empty end,
      if $treasury_result.body.payout_sats_per_window != $treasury_env.payout_sats_per_window then "payout_sats_per_window" else empty end,
      if $treasury_result.body.payout_interval_seconds != $treasury_env.payout_interval_seconds then "payout_interval_seconds" else empty end,
      if $treasury_result.body.require_sellable != $treasury_env.require_sellable then "require_sellable" else empty end,
      if $treasury_result.body.daily_budget_cap_sats != $treasury_env.daily_budget_cap_sats then "daily_budget_cap_sats" else empty end
    ] end;
  def active_alerts:
    ($treasury_result.body.active_continuity_alerts // []);
  def critical_alerts:
    active_alerts | map(select(.severity == "critical"));
  def recent_payout_activity:
    if ($treasury_result.body == null) then null else {
      payout_sats_paid_total: $treasury_result.body.payout_sats_paid_total,
      payout_sats_paid_24h: $treasury_result.body.payout_sats_paid_24h,
      payouts_dispatched_24h: $treasury_result.body.payouts_dispatched_24h,
      payouts_confirmed_24h: $treasury_result.body.payouts_confirmed_24h,
      payouts_failed_24h: $treasury_result.body.payouts_failed_24h,
      payouts_skipped_24h: $treasury_result.body.payouts_skipped_24h,
      eligible_online_payout_targets: $treasury_result.body.eligible_online_payout_targets,
      sellable_pylons_online_now: $treasury_result.body.sellable_pylons_online_now,
      latest_eligible_window_started_at_unix_ms: $treasury_result.body.latest_eligible_window_started_at_unix_ms,
      last_dispatch_at_unix_ms: $treasury_result.body.last_dispatch_at_unix_ms,
      last_confirmed_payout_at_unix_ms: $treasury_result.body.last_confirmed_payout_at_unix_ms,
      eligible_window_lag_ms: $treasury_result.body.eligible_window_lag_ms,
      dispatch_lag_ms: $treasury_result.body.dispatch_lag_ms,
      confirm_lag_ms: $treasury_result.body.confirm_lag_ms,
      skip_reason_metrics_24h: ($treasury_result.body.skip_reason_metrics_24h // []),
      fail_reason_metrics_24h: ($treasury_result.body.fail_reason_metrics_24h // [])
    } end;
  def freshness:
    if ($treasury_result.body == null) then null else {
      snapshot_age_ms: $treasury_result.body.snapshot_age_ms,
      wallet_sync_lag_ms: $treasury_result.body.wallet_sync_lag_ms,
      public_snapshot_generated_at_unix_ms: $treasury_result.body.public_snapshot_generated_at_unix_ms,
      wallet_balance_updated_at_unix_ms: $treasury_result.body.wallet_balance_updated_at_unix_ms
    } end;
  def gates:
    [
      gate(
        "instance_running";
        ($instance_status == "RUNNING");
        (if $instance_status == "RUNNING" then null else "instance_not_running" end);
        {instance_status: $instance_status}
      ),
      gate(
        "service_active";
        ($service_status == "active");
        (if $service_status == "active" then null else "systemd_service_inactive" end);
        {service_status: $service_status}
      ),
      gate(
        "health_endpoint";
        (($health_result.body.ok // false) == true and $health_result.latency_ms <= $verify_health_latency_max_ms);
        (
          if (($health_result.body.ok // false) != true) then "healthz_not_ok"
          elif $health_result.latency_ms > $verify_health_latency_max_ms then "healthz_latency_exceeded"
          else null end
        );
        {latency_ms: $health_result.latency_ms, max_latency_ms: $verify_health_latency_max_ms, body: $health_result.body}
      ),
      gate(
        "stats_endpoint";
        ($stats_result.latency_ms <= $verify_stats_latency_max_ms);
        (if $stats_result.latency_ms > $verify_stats_latency_max_ms then "stats_latency_exceeded" else null end);
        {latency_ms: $stats_result.latency_ms, max_latency_ms: $verify_stats_latency_max_ms}
      )
    ]
    +
    (if $treasury_result.body == null then [
      gate("treasury_status"; true; null; {skipped: true})
    ] else [
      gate(
        "treasury_status";
        ($treasury_result.latency_ms <= $verify_treasury_latency_max_ms);
        (if $treasury_result.latency_ms > $verify_treasury_latency_max_ms then "treasury_status_latency_exceeded" else null end);
        {latency_ms: $treasury_result.latency_ms, max_latency_ms: $verify_treasury_latency_max_ms}
      ),
      gate(
        "treasury_policy_alignment";
        ((policy_drift_fields | length) == 0 and (($treasury_result.body.policy_runtime_status // "unknown") != "blocked"));
        (
          if ((policy_drift_fields | length) > 0) then ("policy_drift:" + (policy_drift_fields | join(",")))
          elif (($treasury_result.body.policy_runtime_status // "unknown") == "blocked") then "policy_runtime_blocked"
          else null end
        );
        {status_policy: treasury_policy_status, env_policy: $treasury_env, drift_fields: policy_drift_fields}
      ),
      gate(
        "treasury_snapshot_freshness";
        (
          (($treasury_result.body.snapshot_age_ms // 0) <= $verify_treasury_snapshot_max_age_ms) and
          (($treasury_result.body.wallet_sync_lag_ms // 0) <= $verify_treasury_wallet_sync_max_lag_ms)
        );
        (
          if (($treasury_result.body.snapshot_age_ms // 0) > $verify_treasury_snapshot_max_age_ms) then "snapshot_age_exceeded"
          elif (($treasury_result.body.wallet_sync_lag_ms // 0) > $verify_treasury_wallet_sync_max_lag_ms) then "wallet_sync_lag_exceeded"
          else null end
        );
        {
          snapshot_age_ms: $treasury_result.body.snapshot_age_ms,
          max_snapshot_age_ms: $verify_treasury_snapshot_max_age_ms,
          wallet_sync_lag_ms: $treasury_result.body.wallet_sync_lag_ms,
          max_wallet_sync_lag_ms: $verify_treasury_wallet_sync_max_lag_ms
        }
      ),
      gate(
        "treasury_payout_continuity";
        ((critical_alerts | length) == 0);
        (if ((critical_alerts | length) > 0) then ("critical_alerts:" + ((critical_alerts | map(.alert_id)) | join(","))) else null end);
        {
          active_alerts: active_alerts,
          eligible_online_payout_targets: $treasury_result.body.eligible_online_payout_targets,
          latest_eligible_window_started_at_unix_ms: $treasury_result.body.latest_eligible_window_started_at_unix_ms,
          last_dispatch_at_unix_ms: $treasury_result.body.last_dispatch_at_unix_ms,
          last_confirmed_payout_at_unix_ms: $treasury_result.body.last_confirmed_payout_at_unix_ms,
          dispatch_lag_ms: $treasury_result.body.dispatch_lag_ms,
          confirm_lag_ms: $treasury_result.body.confirm_lag_ms
        }
      )
    ] end)
  ;
  {
    generated_at: $generated_at,
    vm: $vm,
    instance_status: $instance_status,
    service_status: $service_status,
    image: $image,
    health: $health_result.body,
    stats: $stats_result.body,
    treasury: $treasury_result.body,
    endpoint_latency_ms: {
      healthz: $health_result.latency_ms,
      stats: $stats_result.latency_ms,
      treasury_status: $treasury_result.latency_ms
    },
    treasury_policy_env: $treasury_env,
    treasury_policy_status: treasury_policy_status,
    treasury_recent_payout_activity: recent_payout_activity,
    treasury_freshness: freshness,
    treasury_active_alerts: active_alerts,
    data_mount: $data_mount,
    gates: gates,
    gate_summary: {
      failed_gate_ids: (gates | map(select(.passed | not) | .gate_id)),
      passed: ((gates | map(select(.passed | not)) | length) == 0)
    }
  }' >"$RECEIPT_PATH"

log "Wrote deploy receipt: ${RECEIPT_PATH}"

if ! jq -e '.gate_summary.passed == true' "$RECEIPT_PATH" >/dev/null; then
  jq '.gates | map(select(.passed | not))' "$RECEIPT_PATH" >&2
  die "Nexus deploy verification failed"
fi
