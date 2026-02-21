#!/usr/bin/env bash
set -euo pipefail

RUNTIME_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${RUNTIME_DIR}/docs/reports/restart-reconnect-chaos/${TIMESTAMP}}"
LOG_DIR="${OUTPUT_DIR}/logs"
RESULTS_FILE="${OUTPUT_DIR}/results.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"
REPORT_FILE="${REPORT_FILE:-}"

require_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "error: missing required command: ${command_name}" >&2
    exit 1
  fi
}

require_command jq
require_command mix

mkdir -p "${LOG_DIR}"
: >"${RESULTS_FILE}"

run_case() {
  local case_id="$1"
  local description="$2"
  shift 2

  local log_file="${LOG_DIR}/${case_id}.log"
  local started_at epoch_start epoch_end duration status
  started_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  epoch_start="$(date +%s)"

  if "$@" >"${log_file}" 2>&1; then
    status="passed"
  else
    status="failed"
  fi

  epoch_end="$(date +%s)"
  duration=$((epoch_end - epoch_start))

  jq -n \
    --arg case_id "${case_id}" \
    --arg description "${description}" \
    --arg status "${status}" \
    --arg started_at "${started_at}" \
    --arg command "$(printf '%q ' "$@")" \
    --arg log_path "${log_file}" \
    --argjson duration_seconds "${duration}" \
    '{
      case_id: $case_id,
      description: $description,
      status: $status,
      started_at: $started_at,
      duration_seconds: $duration_seconds,
      command: $command,
      log_path: $log_path
    }' >>"${RESULTS_FILE}"

  echo "[chaos-drill] ${case_id}: ${status} (${duration}s)"
}

(
  cd "${RUNTIME_DIR}"

  run_case \
    "runtime_executor_restart_recovery" \
    "Janitor recovery resumes run execution after executor-loss event and stream resume remains gap-free." \
    mix test test/openagents_runtime/load/runtime_shape_load_test.exs --only chaos_drill

  run_case \
    "khala_reconnect_replay" \
    "Khala replay-on-reconnect covers forced socket drop, stale cursor, and token-expiry reconnect behavior." \
    mix test test/openagents_runtime_web/channels/sync_channel_test.exs --only chaos_drill

  run_case \
    "sync_token_expiry_guard" \
    "Expired sync tokens are rejected deterministically by verifier contract." \
    mix test test/openagents_runtime/sync/jwt_verifier_test.exs --only chaos_drill
)

jq -s \
  --arg timestamp "${TIMESTAMP}" \
  '
  def count_status(s): map(select(.status == s)) | length;
  {
    harness: "oa.runtime_restart_reconnect_chaos.v1",
    timestamp: $timestamp,
    totals: {
      case_count: length,
      passed: count_status("passed"),
      failed: count_status("failed")
    },
    overall_status: (if count_status("failed") > 0 then "failed" else "passed" end),
    cases: .
  }
  ' "${RESULTS_FILE}" >"${SUMMARY_JSON}"

{
  echo "# Runtime/Khala Restart-Reconnect Chaos Drill Report"
  echo
  echo "- Timestamp: ${TIMESTAMP}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Case pass/fail: $(jq -r '.totals.passed' "${SUMMARY_JSON}") passed / $(jq -r '.totals.failed' "${SUMMARY_JSON}") failed"
  echo
  echo "## Case Results"
  echo
  echo "| Case | Status | Duration (s) | Log |"
  echo "| --- | --- | ---: | --- |"
  jq -r '.cases[] | "| `\(.case_id)` | \(.status) | \(.duration_seconds) | `\(.log_path)` |"' "${SUMMARY_JSON}"
  echo
  echo "## Commands"
  echo
  jq -r '.cases[] | "- `\(.case_id)`: `\(.command)`"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

if [[ -n "${REPORT_FILE}" ]]; then
  mkdir -p "$(dirname "${REPORT_FILE}")"
  cp "${SUMMARY_MD}" "${REPORT_FILE}"
fi

echo "[chaos-drill] summary: ${SUMMARY_JSON}"
echo "[chaos-drill] report: ${SUMMARY_MD}"

if [[ "$(jq -r '.overall_status' "${SUMMARY_JSON}")" != "passed" ]]; then
  exit 1
fi
