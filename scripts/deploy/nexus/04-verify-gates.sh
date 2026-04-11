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
VERIFY_LATENCY_SAMPLE_COUNT="${VERIFY_LATENCY_SAMPLE_COUNT:-40}"
VERIFY_HEALTH_LATENCY_P95_MAX_MS="${VERIFY_HEALTH_LATENCY_P95_MAX_MS:-1000}"
VERIFY_HEALTH_LATENCY_P99_MAX_MS="${VERIFY_HEALTH_LATENCY_P99_MAX_MS:-2000}"
VERIFY_STATS_LATENCY_P95_MAX_MS="${VERIFY_STATS_LATENCY_P95_MAX_MS:-1000}"
VERIFY_STATS_LATENCY_P99_MAX_MS="${VERIFY_STATS_LATENCY_P99_MAX_MS:-2000}"
VERIFY_PROVIDER_PRESENCE_LATENCY_P95_MAX_MS="${VERIFY_PROVIDER_PRESENCE_LATENCY_P95_MAX_MS:-1000}"
VERIFY_PROVIDER_PRESENCE_LATENCY_P99_MAX_MS="${VERIFY_PROVIDER_PRESENCE_LATENCY_P99_MAX_MS:-2000}"
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

fetch_json_probe_series() {
  local url="$1"
  local method="${2:-GET}"
  local request_body="${3:-}"
  local sample_count="${4:-$VERIFY_LATENCY_SAMPLE_COUNT}"
  local url_json method_json body_json
  url_json="$(printf '%s' "$url" | jq -Rs .)"
  method_json="$(printf '%s' "$method" | jq -Rs .)"
  body_json="$(printf '%s' "$request_body" | jq -Rs .)"
  ssh_vm "$(cat <<EOF
python3 - <<'PY'
import json
import math
import time
import urllib.request

url = json.loads(${url_json})
method = json.loads(${method_json}).upper()
body = json.loads(${body_json})
sample_count = int(${sample_count})

headers = {}
request_data = None
if method == "POST":
    headers["content-type"] = "application/json"
    request_data = body.encode()

latencies = []
last_body = None
for _ in range(sample_count):
    request = urllib.request.Request(
        url,
        data=request_data,
        headers=headers,
        method=method,
    )
    started = time.time()
    with urllib.request.urlopen(request, timeout=20) as response:
        last_body = json.loads(response.read().decode())
    latencies.append(int((time.time() - started) * 1000))

sorted_latencies = sorted(latencies)

def percentile(values, pct):
    if not values:
        return 0
    index = max(0, math.ceil((pct / 100.0) * len(values)) - 1)
    return values[min(index, len(values) - 1)]

print(json.dumps({
    "body": last_body,
    "sample_count": len(latencies),
    "samples_ms": latencies,
    "latency_ms": {
        "min": min(sorted_latencies) if sorted_latencies else 0,
        "p50": percentile(sorted_latencies, 50),
        "p95": percentile(sorted_latencies, 95),
        "p99": percentile(sorted_latencies, 99),
        "max": max(sorted_latencies) if sorted_latencies else 0,
    },
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

PROVIDER_PRESENCE_DRY_RUN_REQUEST="$(jq -nc --arg relay_url "$NEXUS_PUBLIC_WS_URL" '
  {
    nostr_pubkey_hex: "0000000000000000000000000000000000000000000000000000000000004254",
    session_id: "deploy-verify-heartbeat",
    node_label: "deploy-verify",
    client_version: "nexus-deploy-verifier",
    relay_urls: [$relay_url],
    products: [],
    eligible_product_count: 0,
    ready_model: null,
    runtime_state: "deploy_probe",
    diagnostic_summaries: [],
    hosting_telemetry: {
      captured_at_unix_ms: 0,
      runtime: {
        mode: "offline",
        last_action: null,
        last_error: null,
        degraded_reason_code: null,
        authoritative_status: null,
        authoritative_error_class: null,
        queue_depth: 0,
        online_uptime_seconds: 0,
        inventory_session_started_at_ms: null,
        last_completed_job_at_epoch_ms: null,
        last_authoritative_event_id: null,
        execution_backend_label: "no active inference backend",
        provider_blocker_codes: []
      },
      availability: {
        local_gemma: {
          reachable: false,
          ready: false,
          configured_model: null,
          ready_model: null,
          available_models: [],
          last_error: null,
          last_action: null,
          availability_message: null,
          latency_ms_p50: null
        },
        sandbox: {
          runtimes: [],
          profiles: [],
          last_scan_error: null
        }
      },
      inventory_rows: []
    }
  }'
)"

HEALTH_RESULT="$(fetch_json_probe "http://127.0.0.1:8080/healthz")"
STATS_RESULT="$(fetch_json_probe "http://127.0.0.1:8080/api/stats")"
HEALTH_SERIES_RESULT="$(fetch_json_probe_series "http://127.0.0.1:8080/healthz")"
STATS_SERIES_RESULT="$(fetch_json_probe_series "http://127.0.0.1:8080/api/stats")"
PROVIDER_PRESENCE_SERIES_RESULT="$(fetch_json_probe_series \
  "http://127.0.0.1:8080/api/provider-presence/heartbeat?dry_run=true" \
  "POST" \
  "$PROVIDER_PRESENCE_DRY_RUN_REQUEST"
)"

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
  --argjson health_series_result "$HEALTH_SERIES_RESULT" \
  --argjson stats_series_result "$STATS_SERIES_RESULT" \
  --argjson provider_presence_series_result "$PROVIDER_PRESENCE_SERIES_RESULT" \
  --argjson treasury_result "$TREASURY_RESULT" \
  --argjson treasury_env "$TREASURY_ENV_JSON" \
  --argjson verify_health_latency_max_ms "$VERIFY_HEALTH_LATENCY_MAX_MS" \
  --argjson verify_stats_latency_max_ms "$VERIFY_STATS_LATENCY_MAX_MS" \
  --argjson verify_treasury_latency_max_ms "$VERIFY_TREASURY_LATENCY_MAX_MS" \
  --argjson verify_latency_sample_count "$VERIFY_LATENCY_SAMPLE_COUNT" \
  --argjson verify_health_latency_p95_max_ms "$VERIFY_HEALTH_LATENCY_P95_MAX_MS" \
  --argjson verify_health_latency_p99_max_ms "$VERIFY_HEALTH_LATENCY_P99_MAX_MS" \
  --argjson verify_stats_latency_p95_max_ms "$VERIFY_STATS_LATENCY_P95_MAX_MS" \
  --argjson verify_stats_latency_p99_max_ms "$VERIFY_STATS_LATENCY_P99_MAX_MS" \
  --argjson verify_provider_presence_latency_p95_max_ms "$VERIFY_PROVIDER_PRESENCE_LATENCY_P95_MAX_MS" \
  --argjson verify_provider_presence_latency_p99_max_ms "$VERIFY_PROVIDER_PRESENCE_LATENCY_P99_MAX_MS" \
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
      ),
      gate(
        "health_endpoint_tail_latency";
        (
          (($health_series_result.body.ok // false) == true) and
          (($health_series_result.latency_ms.p95 // 0) <= $verify_health_latency_p95_max_ms) and
          (($health_series_result.latency_ms.p99 // 0) <= $verify_health_latency_p99_max_ms)
        );
        (
          if (($health_series_result.body.ok // false) != true) then "healthz_not_ok"
          elif (($health_series_result.latency_ms.p95 // 0) > $verify_health_latency_p95_max_ms) then "healthz_p95_latency_exceeded"
          elif (($health_series_result.latency_ms.p99 // 0) > $verify_health_latency_p99_max_ms) then "healthz_p99_latency_exceeded"
          else null end
        );
        {
          sample_count: $health_series_result.sample_count,
          latency_ms: $health_series_result.latency_ms,
          max_p95_latency_ms: $verify_health_latency_p95_max_ms,
          max_p99_latency_ms: $verify_health_latency_p99_max_ms
        }
      ),
      gate(
        "stats_endpoint_tail_latency";
        (
          (($stats_series_result.latency_ms.p95 // 0) <= $verify_stats_latency_p95_max_ms) and
          (($stats_series_result.latency_ms.p99 // 0) <= $verify_stats_latency_p99_max_ms)
        );
        (
          if (($stats_series_result.latency_ms.p95 // 0) > $verify_stats_latency_p95_max_ms) then "stats_p95_latency_exceeded"
          elif (($stats_series_result.latency_ms.p99 // 0) > $verify_stats_latency_p99_max_ms) then "stats_p99_latency_exceeded"
          else null end
        );
        {
          sample_count: $stats_series_result.sample_count,
          latency_ms: $stats_series_result.latency_ms,
          max_p95_latency_ms: $verify_stats_latency_p95_max_ms,
          max_p99_latency_ms: $verify_stats_latency_p99_max_ms
        }
      ),
      gate(
        "provider_presence_heartbeat_tail_latency";
        (
          (($provider_presence_series_result.body.status // "") == "online") and
          (($provider_presence_series_result.latency_ms.p95 // 0) <= $verify_provider_presence_latency_p95_max_ms) and
          (($provider_presence_series_result.latency_ms.p99 // 0) <= $verify_provider_presence_latency_p99_max_ms)
        );
        (
          if (($provider_presence_series_result.body.status // "") != "online") then "provider_presence_heartbeat_failed"
          elif (($provider_presence_series_result.latency_ms.p95 // 0) > $verify_provider_presence_latency_p95_max_ms) then "provider_presence_p95_latency_exceeded"
          elif (($provider_presence_series_result.latency_ms.p99 // 0) > $verify_provider_presence_latency_p99_max_ms) then "provider_presence_p99_latency_exceeded"
          else null end
        );
        {
          sample_count: $provider_presence_series_result.sample_count,
          latency_ms: $provider_presence_series_result.latency_ms,
          max_p95_latency_ms: $verify_provider_presence_latency_p95_max_ms,
          max_p99_latency_ms: $verify_provider_presence_latency_p99_max_ms
        }
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
    tail_latency_ms: {
      sample_count: $verify_latency_sample_count,
      healthz: $health_series_result.latency_ms,
      stats: $stats_series_result.latency_ms,
      provider_presence_heartbeat: $provider_presence_series_result.latency_ms
    },
    tail_latency_samples_ms: {
      healthz: $health_series_result.samples_ms,
      stats: $stats_series_result.samples_ms,
      provider_presence_heartbeat: $provider_presence_series_result.samples_ms
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
