#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CATALOG_PATH="${ROOT_DIR}/docs/autopilot/testing/cross-surface-contract-scenarios.json"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/docs/autopilot/testing/reports/cross-surface/${TIMESTAMP}}"
LOG_DIR="${OUTPUT_DIR}/logs"
RESULTS_FILE="${OUTPUT_DIR}/surface-runs.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"
IOS_PROJECT="${IOS_PROJECT:-${ROOT_DIR}/apps/autopilot-ios/Autopilot/Autopilot.xcodeproj}"
IOS_SCHEME="${IOS_SCHEME:-Autopilot}"
IOS_DESTINATION="${IOS_DESTINATION:-platform=iOS Simulator,name=iPhone 17 Pro}"
RUNTIME_API_DIR="${RUNTIME_API_DIR:-${ROOT_DIR}/apps/openagents.com}"
RUNTIME_API_TEST="${RUNTIME_API_TEST:-tests/Feature/Api/RuntimeCodexWorkersApiTest.php}"

require_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "error: missing required command: $command_name" >&2
    exit 1
  fi
}

require_command jq
require_command cargo
require_command xcodebuild
require_command php

if [[ ! -f "${CATALOG_PATH}" ]]; then
  echo "error: scenario catalog missing: ${CATALOG_PATH}" >&2
  exit 1
fi

mkdir -p "${LOG_DIR}"
: >"${RESULTS_FILE}"

SCENARIO_CSV="$(jq -r '[.scenarios[].id] | join(",")' "${CATALOG_PATH}")"

run_surface() {
  local surface="$1"
  shift

  local log_file="${LOG_DIR}/${surface}.log"
  local command_display
  command_display="$(printf '%q ' "$@")"

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

  local scenario_results
  scenario_results="$(printf '%s' "${SCENARIO_CSV}" | tr ',' '\n' | jq -R 'select(length > 0)' | jq -s --arg status "${status}" 'map({id: ., status: $status})')"

  jq -n \
    --arg surface "${surface}" \
    --arg status "${status}" \
    --arg started_at "${started_at}" \
    --arg command "${command_display}" \
    --arg log_path "${log_file}" \
    --argjson duration_seconds "${duration}" \
    --argjson scenario_results "${scenario_results}" \
    '{
      surface: $surface,
      status: $status,
      started_at: $started_at,
      duration_seconds: $duration_seconds,
      command: $command,
      log_path: $log_path,
      scenario_results: $scenario_results
    }' >>"${RESULTS_FILE}"

  echo "[cross-surface] ${surface}: ${status} (${duration}s)"
}

run_surface "web" \
  cargo test -p openagents-web-shell codex_thread::tests

run_surface "desktop" \
  cargo test -p autopilot-desktop runtime_codex_proto::tests

run_surface "runtime-api" \
  bash -lc "cd \"$RUNTIME_API_DIR\" && ./vendor/bin/pest \"$RUNTIME_API_TEST\""

run_surface "ios" \
  xcodebuild test \
    -project "${IOS_PROJECT}" \
    -scheme "${IOS_SCHEME}" \
    -destination "${IOS_DESTINATION}" \
    -only-testing:AutopilotTests

jq -s \
  --slurpfile catalog "${CATALOG_PATH}" \
  --arg timestamp "${TIMESTAMP}" \
  '
  def count_status(s): map(select(.status == s)) | length;
  {
    harness: "oa.cross_surface_contract.v1",
    timestamp: $timestamp,
    catalog: $catalog[0],
    totals: {
      surface_count: length,
      passed: count_status("passed"),
      failed: count_status("failed")
    },
    overall_status: (if count_status("failed") > 0 then "failed" else "passed" end),
    surface_runs: .
  }
  ' "${RESULTS_FILE}" >"${SUMMARY_JSON}"

{
  echo "# Cross-Surface Contract Harness"
  echo
  echo "- Timestamp: ${TIMESTAMP}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Surface pass/fail: $(jq -r '.totals.passed' "${SUMMARY_JSON}") passed / $(jq -r '.totals.failed' "${SUMMARY_JSON}") failed"
  echo
  echo "## Surface Runs"
  echo
  echo "| Surface | Status | Duration (s) | Log |"
  echo "| --- | --- | ---: | --- |"
  jq -r '.surface_runs[] | "| \(.surface) | \(.status) | \(.duration_seconds) | `\(.log_path)` |"' "${SUMMARY_JSON}"
  echo
  echo "## Scenario Suite"
  echo
  jq -r '.catalog.scenarios[] | "- `\(.id)`: \(.description)"' "${SUMMARY_JSON}"
  echo
  echo "## Adapter Commands"
  echo
  jq -r '.surface_runs[] | "- `\(.surface)`: `\(.command)`"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[cross-surface] summary: ${SUMMARY_JSON}"
echo "[cross-surface] report: ${SUMMARY_MD}"

if [[ "$(jq -r '.overall_status' "${SUMMARY_JSON}")" != "passed" ]]; then
  exit 1
fi
