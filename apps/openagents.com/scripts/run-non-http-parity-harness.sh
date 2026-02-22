#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/non-http-parity-harness/${TIMESTAMP}}"
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

  echo "[non-http-harness] ${check_id}: ${status}"
}

run_step "cookie-attributes" "Cookie SameSite/Secure/host-scope semantics" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml web_auth_cookies_include_secure_same_site_and_host_scope

run_step "cors-preflight" "API preflight/OPTIONS behavior and CORS headers" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml api_preflight_options_returns_cors_headers_without_auth

run_step "cache-openapi" "openapi.json cache header contract" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml openapi_route_serves_generated_minified_json

run_step "cache-list-endpoints" "List endpoint cache header contract" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml api_list_routes_default_to_no_store_cache_header

run_step "throttle-key-scope" "Rate-limit key/window behavior (scoped client key)" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml auth_email_throttle_is_scoped_per_client_key

run_step "ws-auth-handshake" "Khala WS auth handshake compatibility" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml sync_token_route_accepts_personal_access_token_auth

run_step "ws-stream-contract" "Khala WS stream contract semantics" \
  cargo test --manifest-path apps/openagents.com/service/Cargo.toml autopilot_stream_route_bootstraps_codex_and_returns_ws_delivery

jq -s \
  --arg generated_at "${TIMESTAMP}" \
  '{
    schema: "openagents.webparity.non_http_harness.v1",
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
  echo "# Non-HTTP Parity Harness"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Check | Status | Description | Log |"
  echo "| --- | --- | --- | --- |"
  jq -r '.checks[] | "| \(.check_id) | \(.status) | \(.description) | `\(.log_path)` |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[non-http-harness] summary: ${SUMMARY_JSON}"
echo "[non-http-harness] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: non-http parity harness failed" >&2
  exit 1
fi
