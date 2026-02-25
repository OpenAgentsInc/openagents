#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

RUNTIME_METRICS_URL="${OA_RUNTIME_METRICS_URL:-}"
CONTROL_STATUS_URL="${OA_CONTROL_STATUS_URL:-}"
OUTPUT_DIR=""

usage() {
  cat <<'EOF'
Usage: scripts/spacetime/run-chaos-drills.sh [--output-dir <dir>] [--runtime-metrics-url <url>] [--control-status-url <url>]

Environment overrides:
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
    --runtime-metrics-url)
      RUNTIME_METRICS_URL="$2"
      shift 2
      ;;
    --control-status-url)
      CONTROL_STATUS_URL="$2"
      shift 2
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
  OUTPUT_DIR="$ROOT_DIR/output/chaos/spacetime/$timestamp"
fi

LOG_DIR="$OUTPUT_DIR/logs"
RESULTS_JSONL="$OUTPUT_DIR/results.jsonl"
SUMMARY_MD="$OUTPUT_DIR/SUMMARY.md"

mkdir -p "$LOG_DIR"
: >"$RESULTS_JSONL"

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

run_scenario() {
  local name="$1"
  local command="$2"
  local log_file="$LOG_DIR/${name}.log"
  local start_epoch end_epoch duration status
  start_epoch="$(date +%s)"
  if bash -lc "cd \"$ROOT_DIR\" && $command" >"$log_file" 2>&1; then
    status="passed"
  else
    status="failed"
  fi
  end_epoch="$(date +%s)"
  duration="$((end_epoch - start_epoch))"

  printf '{"scenario":"%s","status":"%s","duration_seconds":%s,"log_file":"%s"}\n' \
    "$name" "$status" "$duration" "$log_file" >>"$RESULTS_JSONL"

  printf "| %s | %s | %ss | %s |\n" "$name" "$status" "$duration" "$command" >>"$rows_file"

  if [[ "$status" == "failed" ]]; then
    failures="$((failures + 1))"
  fi
  scenario_count="$((scenario_count + 1))"
  total_duration="$((total_duration + duration))"
}

capture_http_snapshot "runtime_metrics_before" "$RUNTIME_METRICS_URL"
capture_http_snapshot "control_status_before" "$CONTROL_STATUS_URL"

rows_file="$(mktemp)"
failures=0
scenario_count=0
total_duration=0

run_scenario "replay_resume_harness" "./scripts/spacetime/replay-resume-parity-harness.sh"
run_scenario "runtime_desktop_e2e_suite" "./scripts/spacetime/runtime-desktop-e2e.sh"
run_scenario "legacy_symbol_guard" "./scripts/spacetime/verify-spacetime-only-symbols.sh"
run_scenario "runtime_publish_observability" "cargo test -p openagents-runtime-service spacetime_publisher::tests::http_publish_failure_queues_outbox_for_retry -- --nocapture"
run_scenario "runtime_retired_spacetime_routes" "cargo test -p openagents-runtime-service retired_spacetime_routes_return_not_found -- --nocapture"
run_scenario "shared_client_stale_cursor" "cargo test -p autopilot-spacetime subscribe_rejects_stale_cursor -- --nocapture"
run_scenario "shared_client_reconnect_helpers" "cargo test -p autopilot-spacetime reconnect_resume_helpers_plan_rebootstrap_and_backoff -- --nocapture"
run_scenario "shared_client_reconnect_storm" "cargo test -p autopilot-spacetime reconnect_storm_resubscribe_keeps_duplicate_delivery_deterministic -- --nocapture"
run_scenario "desktop_reconnect_backoff" "cargo test -p autopilot-desktop reconnect_backoff_grows_and_caps_across_disconnects -- --nocapture"

capture_http_snapshot "runtime_metrics_after" "$RUNTIME_METRICS_URL"
capture_http_snapshot "control_status_after" "$CONTROL_STATUS_URL"

{
  echo "# Spacetime Chaos Drill Summary"
  echo
  echo "- Timestamp (UTC): $timestamp"
  echo "- Output directory: $OUTPUT_DIR"
  echo "- Scenario count: $scenario_count"
  echo "- Failures: $failures"
  echo "- Total duration: ${total_duration}s"
  echo
  echo "| Scenario | Status | Duration | Command |"
  echo "| --- | --- | --- | --- |"
  cat "$rows_file"
} >"$SUMMARY_MD"

rm -f "$rows_file"

echo "Chaos drill artifacts:"
echo "  $RESULTS_JSONL"
echo "  $SUMMARY_MD"
echo "  $LOG_DIR/"

if [[ "$failures" -gt 0 ]]; then
  echo "Chaos drill failed ($failures scenario(s) failed)." >&2
  exit 1
fi

echo "Chaos drill passed."
