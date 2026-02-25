#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

OUTPUT_DIR=""
CONTROL_BASE_URL="${OA_PROD_CONTROL_BASE_URL:-}"
AUTH_TOKEN="${OA_PROD_CONTROL_AUTH_TOKEN:-}"
COHORTS_CSV="${OA_PROD_ROLLOUT_COHORTS:-1,5,10,25,50,100}"
COHORT_SOAK_SECONDS="${OA_PROD_ROLLOUT_COHORT_SOAK_SECONDS:-1800}"
RUNTIME_METRICS_URL="${OA_RUNTIME_METRICS_URL:-}"
CONTROL_STATUS_URL="${OA_CONTROL_STATUS_URL:-}"
SLO_SNAPSHOT_URL="${OA_SPACETIME_SLO_SNAPSHOT_URL:-}"
SLO_MAX_P95_LATENCY_MS="${OA_SPACETIME_SLO_MAX_P95_LATENCY_MS:-600}"
SLO_MAX_ERROR_BUDGET_RATIO="${OA_SPACETIME_SLO_MAX_ERROR_BUDGET_RATIO:-0.020}"
ROLLBACK_DRILL_COMMAND="${OA_SPACETIME_ROLLBACK_DRILL_COMMAND:-./scripts/local-ci.sh canary-drill}"
SKIP_GATES=0
SKIP_ROLLBACK_DRILL=0

usage() {
  cat <<'EOF'
Usage: scripts/spacetime/run-production-phased-rollout.sh [options]

Options:
  --output-dir <dir>               Write artifacts to this directory.
  --control-base-url <url>         Optional production control base URL for live cohort probes.
  --auth-token <token>             Optional bearer token for runtime-routing evaluate probes.
  --cohorts <csv>                  Cohort progression percentages (default: 1,5,10,25,50,100).
  --cohort-soak-seconds <n>        Recommended soak duration between cohorts (default: 1800).
  --runtime-metrics-url <url>      Optional runtime metrics endpoint snapshot URL.
  --control-status-url <url>       Optional control status endpoint snapshot URL.
  --slo-snapshot-url <url>         Optional JSON endpoint with p95/error-budget fields.
  --max-p95-latency-ms <n>         SLO threshold for p95 latency (default: 600).
  --max-error-budget-ratio <ratio> Error budget ratio threshold (default: 0.020).
  --rollback-drill-command <cmd>   Rollback drill command (default: ./scripts/local-ci.sh canary-drill).
  --skip-gates                     Skip local verification gates (not recommended).
  --skip-rollback-drill            Skip rollback drill execution (not recommended).
  -h, --help                       Show this help.

Environment equivalents:
  OA_PROD_CONTROL_BASE_URL
  OA_PROD_CONTROL_AUTH_TOKEN
  OA_PROD_ROLLOUT_COHORTS
  OA_PROD_ROLLOUT_COHORT_SOAK_SECONDS
  OA_RUNTIME_METRICS_URL
  OA_CONTROL_STATUS_URL
  OA_SPACETIME_SLO_SNAPSHOT_URL
  OA_SPACETIME_SLO_MAX_P95_LATENCY_MS
  OA_SPACETIME_SLO_MAX_ERROR_BUDGET_RATIO
  OA_SPACETIME_ROLLBACK_DRILL_COMMAND
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --control-base-url)
      CONTROL_BASE_URL="$2"
      shift 2
      ;;
    --auth-token)
      AUTH_TOKEN="$2"
      shift 2
      ;;
    --cohorts)
      COHORTS_CSV="$2"
      shift 2
      ;;
    --cohort-soak-seconds)
      COHORT_SOAK_SECONDS="$2"
      shift 2
      ;;
    --runtime-metrics-url)
      RUNTIME_METRICS_URL="$2"
      shift 2
      ;;
    --control-status-url)
      CONTROL_STATUS_URL="$2"
      shift 2
      ;;
    --slo-snapshot-url)
      SLO_SNAPSHOT_URL="$2"
      shift 2
      ;;
    --max-p95-latency-ms)
      SLO_MAX_P95_LATENCY_MS="$2"
      shift 2
      ;;
    --max-error-budget-ratio)
      SLO_MAX_ERROR_BUDGET_RATIO="$2"
      shift 2
      ;;
    --rollback-drill-command)
      ROLLBACK_DRILL_COMMAND="$2"
      shift 2
      ;;
    --skip-gates)
      SKIP_GATES=1
      shift
      ;;
    --skip-rollback-drill)
      SKIP_ROLLBACK_DRILL=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

timestamp="$(date -u +"%Y%m%dT%H%M%SZ")"
if [[ -z "$OUTPUT_DIR" ]]; then
  OUTPUT_DIR="$ROOT_DIR/output/canary/spacetime/production-$timestamp"
fi

LOG_DIR="$OUTPUT_DIR/logs"
GATE_RESULTS="$OUTPUT_DIR/gate-results.jsonl"
COHORT_RESULTS="$OUTPUT_DIR/cohort-results.jsonl"
SLO_RESULTS="$OUTPUT_DIR/slo-results.jsonl"
SUMMARY_MD="$OUTPUT_DIR/SUMMARY.md"
ENV_PLAN="$OUTPUT_DIR/cohort-env-sequence.txt"

mkdir -p "$LOG_DIR"
: >"$GATE_RESULTS"
: >"$COHORT_RESULTS"
: >"$SLO_RESULTS"
: >"$ENV_PLAN"

if ! [[ "$COHORT_SOAK_SECONDS" =~ ^[0-9]+$ ]]; then
  echo "Invalid cohort soak seconds: $COHORT_SOAK_SECONDS" >&2
  exit 2
fi

if ! [[ "$SLO_MAX_P95_LATENCY_MS" =~ ^[0-9]+$ ]]; then
  echo "Invalid max p95 latency: $SLO_MAX_P95_LATENCY_MS" >&2
  exit 2
fi

if ! [[ "$SLO_MAX_ERROR_BUDGET_RATIO" =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
  echo "Invalid max error budget ratio: $SLO_MAX_ERROR_BUDGET_RATIO" >&2
  exit 2
fi

declare -a COHORTS=()
IFS=',' read -r -a raw_cohorts <<<"$COHORTS_CSV"
for raw in "${raw_cohorts[@]}"; do
  value="$(echo "$raw" | tr -d '[:space:]')"
  [[ -z "$value" ]] && continue
  if ! [[ "$value" =~ ^[0-9]+$ ]] || (( value < 0 || value > 100 )); then
    echo "Invalid cohort percentage: $raw" >&2
    exit 2
  fi
  COHORTS+=("$value")
done

if [[ "${#COHORTS[@]}" -eq 0 ]]; then
  echo "No valid production cohorts provided." >&2
  exit 2
fi

for cohort in "${COHORTS[@]}"; do
  {
    echo "# Cohort ${cohort}%"
    echo "OA_RUNTIME_CANARY_USER_PERCENT=${cohort}"
    echo "OA_RUNTIME_CANARY_AUTOPILOT_PERCENT=${cohort}"
    echo "OA_RUNTIME_SHADOW_ENABLED=true"
    echo "OA_RUNTIME_SHADOW_SAMPLE_RATE=1.0"
    echo "recommended_soak_seconds=${COHORT_SOAK_SECONDS}"
    echo
  } >>"$ENV_PLAN"
done

gate_failures=0
cohort_failures=0
slo_failures=0
rollback_failures=0
rows_file="$(mktemp)"

run_gate() {
  local name="$1"
  local command="$2"
  local log_file="$LOG_DIR/gate-${name}.log"
  local start_epoch end_epoch duration status
  start_epoch="$(date +%s)"
  if bash -lc "cd \"$ROOT_DIR\" && $command" >"$log_file" 2>&1; then
    status="passed"
  else
    status="failed"
  fi
  end_epoch="$(date +%s)"
  duration="$((end_epoch - start_epoch))"

  printf '{"gate":"%s","status":"%s","duration_seconds":%s,"log_file":"%s"}\n' \
    "$name" "$status" "$duration" "$log_file" >>"$GATE_RESULTS"
  printf "| gate:%s | %s | %ss | %s |\n" "$name" "$status" "$duration" "$command" >>"$rows_file"

  if [[ "$status" == "failed" ]]; then
    gate_failures="$((gate_failures + 1))"
  fi
}

capture_http_snapshot() {
  local label="$1"
  local url="$2"
  [[ -z "$url" ]] && return 0
  local target="$OUTPUT_DIR/${label}.json"
  if curl -fsS "$url" >"$target"; then
    echo "Captured ${label}: $url"
  else
    echo "Failed to capture ${label}: $url" | tee -a "$OUTPUT_DIR/warnings.log"
  fi
}

probe_cohort() {
  local cohort="$1"
  local response_file="$OUTPUT_DIR/cohort-${cohort}.json"
  local status_file="$OUTPUT_DIR/cohort-${cohort}.status"

  if [[ -z "$CONTROL_BASE_URL" || -z "$AUTH_TOKEN" ]]; then
    printf '{"cohort_percent":%s,"status":"skipped","reason":"missing_control_base_or_token"}\n' \
      "$cohort" >>"$COHORT_RESULTS"
    printf "| cohort:%s%% | skipped | 0s | probe skipped (missing control URL/token) |\n" "$cohort" >>"$rows_file"
    return 0
  fi

  local start_epoch end_epoch duration status_code result
  start_epoch="$(date +%s)"
  status_code="$(
    curl -sS \
      -o "$response_file" \
      -w '%{http_code}' \
      -X POST "${CONTROL_BASE_URL%/}/api/v1/control/runtime-routing/evaluate" \
      -H "content-type: application/json" \
      -H "authorization: Bearer ${AUTH_TOKEN}" \
      --data "{\"thread_id\":\"production-rollout-thread-${cohort}\",\"cohort_key\":\"user:production-rollout-${cohort}\"}"
  )"
  end_epoch="$(date +%s)"
  duration="$((end_epoch - start_epoch))"

  echo "$status_code" >"$status_file"
  if [[ "$status_code" == "200" ]]; then
    result="passed"
  else
    result="failed"
    cohort_failures="$((cohort_failures + 1))"
  fi

  printf '{"cohort_percent":%s,"status":"%s","http_status":"%s","duration_seconds":%s,"response_file":"%s"}\n' \
    "$cohort" "$result" "$status_code" "$duration" "$response_file" >>"$COHORT_RESULTS"
  printf "| cohort:%s%% | %s | %ss | runtime-routing evaluate (HTTP %s) |\n" \
    "$cohort" "$result" "$duration" "$status_code" >>"$rows_file"
}

check_slo_snapshot() {
  local label="$1"
  local snapshot_file="$OUTPUT_DIR/slo-${label}.json"
  if [[ -z "$SLO_SNAPSHOT_URL" ]]; then
    printf '{"label":"%s","status":"skipped","reason":"missing_slo_snapshot_url"}\n' "$label" >>"$SLO_RESULTS"
    printf "| slo:%s | skipped | 0s | no slo endpoint configured |\n" "$label" >>"$rows_file"
    return 0
  fi

  if ! command -v jq >/dev/null 2>&1; then
    printf '{"label":"%s","status":"failed","reason":"jq_not_found"}\n' "$label" >>"$SLO_RESULTS"
    printf "| slo:%s | failed | 0s | jq is required for SLO parsing |\n" "$label" >>"$rows_file"
    slo_failures="$((slo_failures + 1))"
    return 0
  fi

  local start_epoch end_epoch duration
  start_epoch="$(date +%s)"
  if ! curl -fsS "$SLO_SNAPSHOT_URL" >"$snapshot_file"; then
    end_epoch="$(date +%s)"
    duration="$((end_epoch - start_epoch))"
    printf '{"label":"%s","status":"failed","reason":"snapshot_fetch_failed","duration_seconds":%s}\n' \
      "$label" "$duration" >>"$SLO_RESULTS"
    printf "| slo:%s | failed | %ss | failed to fetch snapshot |\n" "$label" "$duration" >>"$rows_file"
    slo_failures="$((slo_failures + 1))"
    return 0
  fi
  end_epoch="$(date +%s)"
  duration="$((end_epoch - start_epoch))"

  local p95_value error_budget_value
  p95_value="$(jq -r '.p95_latency_ms // .sync.p95_latency_ms // empty' "$snapshot_file")"
  error_budget_value="$(jq -r '.error_budget_ratio // .sync.error_budget_ratio // empty' "$snapshot_file")"
  if [[ -z "$p95_value" || -z "$error_budget_value" ]]; then
    printf '{"label":"%s","status":"failed","reason":"missing_metrics_fields","duration_seconds":%s}\n' \
      "$label" "$duration" >>"$SLO_RESULTS"
    printf "| slo:%s | failed | %ss | snapshot missing p95/error fields |\n" "$label" "$duration" >>"$rows_file"
    slo_failures="$((slo_failures + 1))"
    return 0
  fi

  local status="passed"
  if ! awk "BEGIN { exit !($p95_value <= $SLO_MAX_P95_LATENCY_MS) }"; then
    status="failed"
  fi
  if ! awk "BEGIN { exit !($error_budget_value <= $SLO_MAX_ERROR_BUDGET_RATIO) }"; then
    status="failed"
  fi

  printf '{"label":"%s","status":"%s","duration_seconds":%s,"p95_latency_ms":%s,"error_budget_ratio":%s,"snapshot_file":"%s"}\n' \
    "$label" "$status" "$duration" "$p95_value" "$error_budget_value" "$snapshot_file" >>"$SLO_RESULTS"
  printf "| slo:%s | %s | %ss | p95=%sms threshold<=%s, error_budget=%s threshold<=%s |\n" \
    "$label" "$status" "$duration" "$p95_value" "$SLO_MAX_P95_LATENCY_MS" "$error_budget_value" "$SLO_MAX_ERROR_BUDGET_RATIO" >>"$rows_file"

  if [[ "$status" == "failed" ]]; then
    slo_failures="$((slo_failures + 1))"
  fi
}

run_rollback_drill() {
  local start_epoch end_epoch duration status
  local log_file="$LOG_DIR/rollback-drill.log"

  if [[ "$SKIP_ROLLBACK_DRILL" -eq 1 ]]; then
    printf '{"gate":"rollback_drill","status":"skipped","reason":"--skip-rollback-drill"}\n' >>"$GATE_RESULTS"
    printf "| gate:rollback_drill | skipped | 0s | --skip-rollback-drill |\n" >>"$rows_file"
    return 0
  fi

  start_epoch="$(date +%s)"
  if bash -lc "cd \"$ROOT_DIR\" && $ROLLBACK_DRILL_COMMAND" >"$log_file" 2>&1; then
    status="passed"
  else
    status="failed"
  fi
  end_epoch="$(date +%s)"
  duration="$((end_epoch - start_epoch))"

  printf '{"gate":"rollback_drill","status":"%s","duration_seconds":%s,"log_file":"%s","command":"%s"}\n' \
    "$status" "$duration" "$log_file" "$ROLLBACK_DRILL_COMMAND" >>"$GATE_RESULTS"
  printf "| gate:rollback_drill | %s | %ss | %s |\n" "$status" "$duration" "$ROLLBACK_DRILL_COMMAND" >>"$rows_file"

  if [[ "$status" == "failed" ]]; then
    rollback_failures="$((rollback_failures + 1))"
  fi
}

capture_http_snapshot "runtime_metrics_before" "$RUNTIME_METRICS_URL"
capture_http_snapshot "control_status_before" "$CONTROL_STATUS_URL"
check_slo_snapshot "baseline"

if [[ "$SKIP_GATES" -eq 0 ]]; then
  run_gate "spacetime-provision-prod" "./scripts/spacetime/provision-check.sh prod"
  run_gate "spacetime-replay-resume" "./scripts/local-ci.sh spacetime-replay-resume"
  run_gate "spacetime-chaos" "./scripts/local-ci.sh spacetime-chaos"
  run_gate "sync-security" "./scripts/local-ci.sh sync-security"
else
  printf '{"gate":"all","status":"skipped","reason":"--skip-gates"}\n' >>"$GATE_RESULTS"
  printf "| gate:all | skipped | 0s | --skip-gates |\n" >>"$rows_file"
fi

for cohort in "${COHORTS[@]}"; do
  capture_http_snapshot "runtime_metrics_before_${cohort}" "$RUNTIME_METRICS_URL"
  capture_http_snapshot "control_status_before_${cohort}" "$CONTROL_STATUS_URL"
  probe_cohort "$cohort"
  check_slo_snapshot "cohort_${cohort}"
  capture_http_snapshot "runtime_metrics_after_${cohort}" "$RUNTIME_METRICS_URL"
  capture_http_snapshot "control_status_after_${cohort}" "$CONTROL_STATUS_URL"
done

run_rollback_drill

capture_http_snapshot "runtime_metrics_after" "$RUNTIME_METRICS_URL"
capture_http_snapshot "control_status_after" "$CONTROL_STATUS_URL"
check_slo_snapshot "post_rollback_drill"

decision="allow"
if (( gate_failures > 0 || cohort_failures > 0 || slo_failures > 0 || rollback_failures > 0 )); then
  decision="block"
fi

{
  echo "# Spacetime Production Phased Rollout Summary"
  echo
  echo "- Timestamp (UTC): $timestamp"
  echo "- Output directory: $OUTPUT_DIR"
  echo "- Gate failures: $gate_failures"
  echo "- Cohort probe failures: $cohort_failures"
  echo "- SLO failures: $slo_failures"
  echo "- Rollback drill failures: $rollback_failures"
  echo "- Recommended soak per cohort: ${COHORT_SOAK_SECONDS}s"
  echo "- SLO thresholds: p95<=${SLO_MAX_P95_LATENCY_MS}ms, error_budget<=${SLO_MAX_ERROR_BUDGET_RATIO}"
  echo "- Decision: $decision"
  echo
  echo "## Cohort Sequence"
  echo
  echo "$COHORTS_CSV"
  echo
  echo "## Cohort Env Sequence"
  echo
  echo "See: $ENV_PLAN"
  echo
  echo "## Execution Table"
  echo
  echo "| Step | Status | Duration | Detail |"
  echo "| --- | --- | --- | --- |"
  cat "$rows_file"
} >"$SUMMARY_MD"

rm -f "$rows_file"

echo "Production rollout artifacts:"
echo "  $GATE_RESULTS"
echo "  $COHORT_RESULTS"
echo "  $SLO_RESULTS"
echo "  $ENV_PLAN"
echo "  $SUMMARY_MD"
echo "  $LOG_DIR/"

if [[ "$decision" == "block" ]]; then
  echo "Production phased rollout blocked." >&2
  exit 1
fi

echo "Production phased rollout gates passed."
