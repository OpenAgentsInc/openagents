#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
APP_DIR="${ROOT_DIR}/apps/openagents.com"
REQUESTS_FILE_DEFAULT="${APP_DIR}/docs/parity-manifests/staging-dual-run-requests.json"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${APP_DIR}/storage/app/staging-dual-run/${TIMESTAMP}}"
RESULTS_JSONL="${OUTPUT_DIR}/request-results.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

RUST_BASE_URL="${RUST_BASE_URL:-}"
LEGACY_BASE_URL="${LEGACY_BASE_URL:-}"
AUTH_TOKEN="${AUTH_TOKEN:-}"
REQUESTS_FILE="${REQUESTS_FILE:-${REQUESTS_FILE_DEFAULT}}"

require_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "error: missing required command: ${command_name}" >&2
    exit 1
  fi
}

require_command curl
require_command jq
require_command diff
require_command sha256sum

if [[ -z "${RUST_BASE_URL}" ]]; then
  echo "error: RUST_BASE_URL is required" >&2
  exit 1
fi
if [[ -z "${LEGACY_BASE_URL}" ]]; then
  echo "error: LEGACY_BASE_URL is required" >&2
  exit 1
fi
if [[ ! -f "${REQUESTS_FILE}" ]]; then
  echo "error: request manifest missing: ${REQUESTS_FILE}" >&2
  exit 1
fi

mkdir -p "${OUTPUT_DIR}/responses" "${OUTPUT_DIR}/normalized" "${OUTPUT_DIR}/diffs"
: >"${RESULTS_JSONL}"

trim_base_url() {
  local raw="$1"
  local trimmed
  trimmed="$(printf '%s' "${raw}" | sed 's/[[:space:]]*$//' | sed 's#/$##')"
  if [[ -z "${trimmed}" ]]; then
    echo "error: empty base URL" >&2
    exit 1
  fi
  printf '%s' "${trimmed}"
}

RUST_BASE_URL="$(trim_base_url "${RUST_BASE_URL}")"
LEGACY_BASE_URL="$(trim_base_url "${LEGACY_BASE_URL}")"

canonicalize_json() {
  local source_path="$1"
  local target_path="$2"
  jq -S '
    del(.. | .generated_at?) |
    del(.. | .generatedAt?) |
    del(.. | .request_id?) |
    del(.. | .requestId?)
  ' "${source_path}" >"${target_path}"
}

perform_request() {
  local target_name="$1"
  local base_url="$2"
  local request_id="$3"
  local method="$4"
  local path="$5"
  local response_kind="$6"
  local requires_auth="$7"
  local body_payload="$8"
  local headers_json="$9"

  local url="${base_url}${path}"
  local body_path="${OUTPUT_DIR}/responses/${request_id}.${target_name}.body"
  local normalized_path="${OUTPUT_DIR}/normalized/${request_id}.${target_name}.normalized"
  local headers_path="${OUTPUT_DIR}/responses/${request_id}.${target_name}.headers"

  local -a curl_args
  curl_args=(
    -sS
    -X "${method}"
    "${url}"
    -D "${headers_path}"
    -o "${body_path}"
    -w "%{http_code}"
  )

  if [[ "${requires_auth}" == "true" && -n "${AUTH_TOKEN}" ]]; then
    curl_args+=( -H "authorization: Bearer ${AUTH_TOKEN}" )
  fi

  if [[ -n "${headers_json}" && "${headers_json}" != "null" ]]; then
    while IFS= read -r header_line; do
      [[ -z "${header_line}" ]] && continue
      curl_args+=( -H "${header_line}" )
    done < <(jq -r 'to_entries[] | "\(.key): \(.value)"' <<<"${headers_json}")
  fi

  if [[ -n "${body_payload}" && "${body_payload}" != "null" ]]; then
    curl_args+=( -H "content-type: application/json" --data "${body_payload}" )
  fi

  local http_code
  if ! http_code="$(curl "${curl_args[@]}")"; then
    http_code="000"
  fi

  if [[ "${response_kind}" == "json" ]]; then
    if jq -e . "${body_path}" >/dev/null 2>&1; then
      canonicalize_json "${body_path}" "${normalized_path}"
    else
      cp "${body_path}" "${normalized_path}"
    fi
  else
    cp "${body_path}" "${normalized_path}"
  fi

  local body_hash
  body_hash="$(sha256sum "${normalized_path}" | awk '{print $1}')"

  jq -n \
    --arg http_code "${http_code}" \
    --arg body_path "${body_path}" \
    --arg normalized_path "${normalized_path}" \
    --arg body_hash "${body_hash}" \
    '{
      status_code: ($http_code | tonumber),
      body_path: $body_path,
      normalized_path: $normalized_path,
      body_hash: $body_hash
    }'
}

critical_mismatch_count=0
mismatch_count=0
skip_count=0

while IFS= read -r request_b64; do
  request_json="$(printf '%s' "${request_b64}" | base64 --decode)"

  request_id="$(jq -r '.id' <<<"${request_json}")"
  method="$(jq -r '.method // "GET"' <<<"${request_json}")"
  path="$(jq -r '.path' <<<"${request_json}")"
  response_kind="$(jq -r '.response_kind // "json"' <<<"${request_json}")"
  requires_auth="$(jq -r '(.requires_auth // false) | tostring' <<<"${request_json}")"
  critical="$(jq -r '(.critical // false) | tostring' <<<"${request_json}")"
  body_payload="$(jq -c '.body // empty' <<<"${request_json}")"
  headers_json="$(jq -c '.headers // empty' <<<"${request_json}")"

  if [[ "${requires_auth}" == "true" && -z "${AUTH_TOKEN}" ]]; then
    skip_count=$((skip_count + 1))
    jq -n \
      --arg request_id "${request_id}" \
      --arg method "${method}" \
      --arg path "${path}" \
      --arg response_kind "${response_kind}" \
      --arg critical "${critical}" \
      '{
        request_id: $request_id,
        method: $method,
        path: $path,
        response_kind: $response_kind,
        critical: ($critical == "true"),
        status: "skipped",
        reason: "auth_token_missing"
      }' >>"${RESULTS_JSONL}"
    continue
  fi

  rust_result="$(perform_request "rust" "${RUST_BASE_URL}" "${request_id}" "${method}" "${path}" "${response_kind}" "${requires_auth}" "${body_payload}" "${headers_json}")"
  legacy_result="$(perform_request "legacy" "${LEGACY_BASE_URL}" "${request_id}" "${method}" "${path}" "${response_kind}" "${requires_auth}" "${body_payload}" "${headers_json}")"

  rust_status="$(jq -r '.status_code' <<<"${rust_result}")"
  legacy_status="$(jq -r '.status_code' <<<"${legacy_result}")"

  rust_normalized="$(jq -r '.normalized_path' <<<"${rust_result}")"
  legacy_normalized="$(jq -r '.normalized_path' <<<"${legacy_result}")"

  diff_path="${OUTPUT_DIR}/diffs/${request_id}.diff"
  status="pass"
  reason=""

  if [[ "${rust_status}" != "${legacy_status}" ]]; then
    status="fail"
    reason="status_mismatch"
    : >"${diff_path}"
  elif diff -u "${legacy_normalized}" "${rust_normalized}" >"${diff_path}"; then
    rm -f "${diff_path}"
  else
    status="fail"
    reason="body_mismatch"
  fi

  if [[ "${status}" == "fail" ]]; then
    mismatch_count=$((mismatch_count + 1))
    if [[ "${critical}" == "true" ]]; then
      critical_mismatch_count=$((critical_mismatch_count + 1))
    fi
  fi

  jq -n \
    --arg request_id "${request_id}" \
    --arg method "${method}" \
    --arg path "${path}" \
    --arg response_kind "${response_kind}" \
    --arg critical "${critical}" \
    --arg status "${status}" \
    --arg reason "${reason}" \
    --arg rust_base "${RUST_BASE_URL}" \
    --arg legacy_base "${LEGACY_BASE_URL}" \
    --arg diff_path "$( [[ -f "${diff_path}" ]] && echo "${diff_path}" || echo "" )" \
    --argjson rust_result "${rust_result}" \
    --argjson legacy_result "${legacy_result}" \
    '{
      request_id: $request_id,
      method: $method,
      path: $path,
      response_kind: $response_kind,
      critical: ($critical == "true"),
      status: $status,
      reason: (if $reason == "" then null else $reason end),
      rust: {
        base_url: $rust_base,
        status_code: $rust_result.status_code,
        body_hash: $rust_result.body_hash,
        body_path: $rust_result.body_path,
        normalized_path: $rust_result.normalized_path
      },
      legacy: {
        base_url: $legacy_base,
        status_code: $legacy_result.status_code,
        body_hash: $legacy_result.body_hash,
        body_path: $legacy_result.body_path,
        normalized_path: $legacy_result.normalized_path
      },
      diff_path: (if $diff_path == "" then null else $diff_path end)
    }' >>"${RESULTS_JSONL}"

done < <(jq -r '.requests[] | @base64' "${REQUESTS_FILE}")

jq -s \
  --arg generated_at "${TIMESTAMP}" \
  --arg rust_base_url "${RUST_BASE_URL}" \
  --arg legacy_base_url "${LEGACY_BASE_URL}" \
  --arg requests_file "${REQUESTS_FILE}" \
  '
  def count_status(s): map(select(.status == s)) | length;
  {
    schema: "openagents.webparity.staging_dual_run_diff.v1",
    generated_at: $generated_at,
    rust_base_url: $rust_base_url,
    legacy_base_url: $legacy_base_url,
    requests_file: $requests_file,
    totals: {
      request_count: length,
      passed: count_status("pass"),
      failed: count_status("fail"),
      skipped: count_status("skipped"),
      critical_failed: map(select(.status == "fail" and .critical == true)) | length
    },
    overall_status: (
      if (map(select(.status == "fail" and .critical == true)) | length) > 0 then "failed"
      elif count_status("fail") > 0 then "mismatch"
      else "passed"
      end
    ),
    requests: .
  }
  ' "${RESULTS_JSONL}" >"${SUMMARY_JSON}"

{
  echo "# Staging Dual-Run Shadow Diff Report"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Rust base URL: ${RUST_BASE_URL}"
  echo "- Legacy base URL: ${LEGACY_BASE_URL}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail / $(jq -r '.totals.skipped' "${SUMMARY_JSON}") skipped"
  echo
  echo "| Request | Method | Path | Critical | Status | Reason | Diff |"
  echo "| --- | --- | --- | --- | --- | --- | --- |"
  jq -r '.requests[] | "| \(.request_id) | \(.method) | \(.path) | \(.critical) | \(.status) | \(.reason // "") | \((.diff_path // "") | if . == "" then "" else "`" + . + "`" end) |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[staging-dual-run] summary: ${SUMMARY_JSON}"
echo "[staging-dual-run] report: ${SUMMARY_MD}"

if [[ "${critical_mismatch_count}" -gt 0 ]]; then
  echo "error: detected ${critical_mismatch_count} critical mismatch(es)" >&2
  exit 1
fi

if [[ "${mismatch_count}" -gt 0 ]]; then
  echo "warning: detected ${mismatch_count} non-critical mismatch(es)" >&2
fi

if [[ "${skip_count}" -gt 0 ]]; then
  echo "warning: skipped ${skip_count} auth-required request(s) because AUTH_TOKEN was not provided" >&2
fi
