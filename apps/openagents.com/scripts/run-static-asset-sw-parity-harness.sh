#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/static-asset-sw-parity-harness/${TIMESTAMP}}"
RESULTS_JSONL="${OUTPUT_DIR}/checks.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

mkdir -p "${OUTPUT_DIR}"
: >"${RESULTS_JSONL}"

overall_failed=0

run_step() {
  local check_id="$1"
  local description="$2"
  shift 2

  local log_path="${OUTPUT_DIR}/${check_id}.log"
  local started_at ended_at status reason
  started_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  if (
    cd "${ROOT_DIR}"
    "$@"
  ) >"${log_path}" 2>&1; then
    status="pass"
    reason=""
  else
    status="fail"
    reason="command_failed"
    overall_failed=1
  fi

  ended_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  jq -n \
    --arg check_id "${check_id}" \
    --arg description "${description}" \
    --arg command "$(printf '%q ' "$@")" \
    --arg status "${status}" \
    --arg reason "${reason}" \
    --arg started_at "${started_at}" \
    --arg ended_at "${ended_at}" \
    --arg log_path "${log_path}" \
    '{
      check_id: $check_id,
      description: $description,
      command: $command,
      status: $status,
      reason: (if $reason == "" then null else $reason end),
      started_at: $started_at,
      ended_at: $ended_at,
      log_path: $log_path
    }' >>"${RESULTS_JSONL}"

  echo "[static-asset-sw-harness] ${check_id}: ${status}"
}

run_step "landing-healthz" "Landing-mode service keeps health endpoint stable" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml healthz_route_returns_ok

run_step "landing-readiness" "Landing-mode readiness gate remains deterministic" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml readiness_route_is_ready_when_static_dir_exists

run_step "etag-openapi-conditional" "OpenAPI route emits ETag and honors If-None-Match" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml openapi_route_supports_etag_conditional_get

jq -s \
  --arg generated_at "${TIMESTAMP}" \
  '{
    schema: "openagents.webparity.static_asset_sw_harness.v1",
    generated_at: $generated_at,
    totals: {
      check_count: length,
      passed: (map(select(.status == "pass")) | length),
      failed: (map(select(.status == "fail")) | length)
    },
    overall_status: (if (map(select(.status == "fail")) | length) > 0 then "failed" else "passed" end),
    checks: .
  }' "${RESULTS_JSONL}" >"${SUMMARY_JSON}"

{
  echo "# Static Asset + Service Worker Parity Harness"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Check | Status | Description | Log |"
  echo "| --- | --- | --- | --- |"
  jq -r '.checks[] | "| \(.check_id) | \(.status) | \(.description) | `\(.log_path)` |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[static-asset-sw-harness] summary: ${SUMMARY_JSON}"
echo "[static-asset-sw-harness] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: static asset/service-worker parity harness failed" >&2
  exit 1
fi
