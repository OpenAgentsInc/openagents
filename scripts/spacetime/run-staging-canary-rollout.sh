#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

OUTPUT_DIR=""
CONTROL_BASE_URL="${OA_STAGING_CONTROL_BASE_URL:-}"
AUTH_TOKEN="${OA_STAGING_CONTROL_AUTH_TOKEN:-}"
COHORTS_CSV="${OA_STAGING_CANARY_COHORTS:-5,10,25,50,100}"
COHORT_SOAK_SECONDS="${OA_STAGING_CANARY_COHORT_SOAK_SECONDS:-900}"
RUNTIME_METRICS_URL="${OA_RUNTIME_METRICS_URL:-}"
CONTROL_STATUS_URL="${OA_CONTROL_STATUS_URL:-}"
SKIP_GATES=0

usage() {
  cat <<'EOF'
Usage: scripts/spacetime/run-staging-canary-rollout.sh [options]

Options:
  --output-dir <dir>          Write artifacts to this directory.
  --control-base-url <url>    Optional staging control base URL for live cohort probes.
  --auth-token <token>        Optional bearer token for runtime-routing evaluate probes.
  --cohorts <csv>             Canary progression percentages (default: 5,10,25,50,100).
  --cohort-soak-seconds <n>   Recommended soak duration between cohorts (default: 900).
  --runtime-metrics-url <url> Optional runtime metrics endpoint snapshot URL.
  --control-status-url <url>  Optional control status endpoint snapshot URL.
  --skip-gates                Skip local verification gates (not recommended).
  -h, --help                  Show this help.

Environment equivalents:
  OA_STAGING_CONTROL_BASE_URL
  OA_STAGING_CONTROL_AUTH_TOKEN
  OA_STAGING_CANARY_COHORTS
  OA_STAGING_CANARY_COHORT_SOAK_SECONDS
  OA_RUNTIME_METRICS_URL
  OA_CONTROL_STATUS_URL
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
    --skip-gates)
      SKIP_GATES=1
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
  OUTPUT_DIR="$ROOT_DIR/output/canary/spacetime/staging-$timestamp"
fi

LOG_DIR="$OUTPUT_DIR/logs"
GATE_RESULTS="$OUTPUT_DIR/gate-results.jsonl"
COHORT_RESULTS="$OUTPUT_DIR/cohort-results.jsonl"
SUMMARY_MD="$OUTPUT_DIR/SUMMARY.md"
ENV_PLAN="$OUTPUT_DIR/canary-env-sequence.txt"

mkdir -p "$LOG_DIR"
: >"$GATE_RESULTS"
: >"$COHORT_RESULTS"
: >"$ENV_PLAN"

if ! [[ "$COHORT_SOAK_SECONDS" =~ ^[0-9]+$ ]]; then
  echo "Invalid cohort soak seconds: $COHORT_SOAK_SECONDS" >&2
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
  echo "No valid canary cohorts provided." >&2
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
      --data "{\"thread_id\":\"staging-canary-thread-${cohort}\",\"cohort_key\":\"user:staging-canary-${cohort}\"}"
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

capture_http_snapshot "runtime_metrics_before" "$RUNTIME_METRICS_URL"
capture_http_snapshot "control_status_before" "$CONTROL_STATUS_URL"

if [[ "$SKIP_GATES" -eq 0 ]]; then
  run_gate "spacetime-provision-staging" "./scripts/spacetime/provision-check.sh staging"
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
  capture_http_snapshot "runtime_metrics_after_${cohort}" "$RUNTIME_METRICS_URL"
  capture_http_snapshot "control_status_after_${cohort}" "$CONTROL_STATUS_URL"
done

capture_http_snapshot "runtime_metrics_after" "$RUNTIME_METRICS_URL"
capture_http_snapshot "control_status_after" "$CONTROL_STATUS_URL"

decision="allow"
if (( gate_failures > 0 || cohort_failures > 0 )); then
  decision="block"
fi

{
  echo "# Staging Spacetime Canary Rollout Summary"
  echo
  echo "- Timestamp (UTC): $timestamp"
  echo "- Output directory: $OUTPUT_DIR"
  echo "- Gate failures: $gate_failures"
  echo "- Cohort probe failures: $cohort_failures"
  echo "- Recommended soak per cohort: ${COHORT_SOAK_SECONDS}s"
  echo "- Decision: $decision"
  echo
  echo "## Cohort Sequence"
  echo
  echo "$COHORTS_CSV"
  echo
  echo "## Canary Env Sequence"
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

echo "Staging canary artifacts:"
echo "  $GATE_RESULTS"
echo "  $COHORT_RESULTS"
echo "  $ENV_PLAN"
echo "  $SUMMARY_MD"
echo "  $LOG_DIR/"

if [[ "$decision" == "block" ]]; then
  echo "Staging canary rollout blocked." >&2
  exit 1
fi

echo "Staging canary rollout gates passed."
