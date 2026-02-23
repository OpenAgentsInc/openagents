#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/htmx-route-group-canary/${TIMESTAMP}}"
RESULTS_JSONL="${OUTPUT_DIR}/checks.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

BASE_URL="${BASE_URL:-https://staging.openagents.com}"
BASE_URL="${BASE_URL%/}"
CONTROL_ACCESS_TOKEN="${CONTROL_ACCESS_TOKEN:-}"
DOMAINS_CSV="${DOMAINS:-auth_entry,account_settings_admin,billing_l402,chat_pilot}"
CURL_TIMEOUT_SECONDS="${CURL_TIMEOUT_SECONDS:-15}"

if [[ -z "${CONTROL_ACCESS_TOKEN}" ]]; then
  echo "error: CONTROL_ACCESS_TOKEN is required" >&2
  exit 1
fi

for cmd in curl jq; do
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "error: missing required command: ${cmd}" >&2
    exit 1
  fi
done

mkdir -p "${OUTPUT_DIR}"
: >"${RESULTS_JSONL}"

overall_failed=0

record_check() {
  local check_id="$1"
  local status="$2"
  local detail="$3"
  local artifact="${4:-}"

  jq -n \
    --arg check_id "${check_id}" \
    --arg status "${status}" \
    --arg detail "${detail}" \
    --arg artifact "${artifact}" \
    '{
      check_id: $check_id,
      status: $status,
      detail: $detail,
      artifact: (if $artifact == "" then null else $artifact end),
      recorded_at: (now | todateiso8601)
    }' >>"${RESULTS_JSONL}"

  echo "[htmx-route-group-canary] ${check_id}: ${status}"
  if [[ "${status}" == "fail" ]]; then
    overall_failed=1
  fi
}

control_request() {
  local method="$1"
  local endpoint="$2"
  local payload="$3"
  local output_path="$4"

  if [[ -n "${payload}" ]]; then
    curl -sS --max-time "${CURL_TIMEOUT_SECONDS}" \
      -o "${output_path}" \
      -w "%{http_code}" \
      -X "${method}" \
      "${BASE_URL}${endpoint}" \
      -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
      -H "content-type: application/json" \
      --data "${payload}"
    return
  fi

  curl -sS --max-time "${CURL_TIMEOUT_SECONDS}" \
    -o "${output_path}" \
    -w "%{http_code}" \
    -X "${method}" \
    "${BASE_URL}${endpoint}" \
    -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}"
}

status_snapshot() {
  local name="$1"
  local output_file="${OUTPUT_DIR}/status-${name}.json"
  local http_status
  http_status="$(control_request "GET" "/api/v1/control/route-split/status" "" "${output_file}")"
  if [[ "${http_status}" != "200" ]]; then
    record_check "status-${name}" "fail" "status request failed (http=${http_status})" "${output_file}"
    return 1
  fi
  record_check "status-${name}" "pass" "status snapshot captured" "${output_file}"
}

status_assert_domain_mode() {
  local check_id="$1"
  local domain="$2"
  local expected_mode="$3"
  local mode_file="${OUTPUT_DIR}/${check_id}.json"
  local http_status
  http_status="$(control_request "GET" "/api/v1/control/route-split/status" "" "${mode_file}")"
  if [[ "${http_status}" != "200" ]]; then
    record_check "${check_id}" "fail" "status request failed (http=${http_status})" "${mode_file}"
    return
  fi

  local actual
  actual="$(jq -r --arg domain "${domain}" '.data.htmx_domain_overrides[$domain] // "<none>"' "${mode_file}")"
  if [[ "${actual}" == "${expected_mode}" ]]; then
    record_check "${check_id}" "pass" "domain=${domain} mode=${actual}" "${mode_file}"
  else
    record_check "${check_id}" "fail" "domain=${domain} expected=${expected_mode} actual=${actual}" "${mode_file}"
  fi
}

status_assert_domain_mode_matches_rollback() {
  local check_id="$1"
  local domain="$2"
  local mode_file="${OUTPUT_DIR}/${check_id}.json"
  local http_status
  http_status="$(control_request "GET" "/api/v1/control/route-split/status" "" "${mode_file}")"
  if [[ "${http_status}" != "200" ]]; then
    record_check "${check_id}" "fail" "status request failed (http=${http_status})" "${mode_file}"
    return
  fi

  local actual rollback
  actual="$(jq -r --arg domain "${domain}" '.data.htmx_domain_overrides[$domain] // "<none>"' "${mode_file}")"
  rollback="$(jq -r --arg domain "${domain}" '.data.htmx_rollback_matrix[$domain] // "<none>"' "${mode_file}")"

  if [[ "${actual}" == "${rollback}" && "${rollback}" != "<none>" ]]; then
    record_check "${check_id}" "pass" "domain=${domain} mode=${actual} rollback=${rollback}" "${mode_file}"
  else
    record_check "${check_id}" "fail" "domain=${domain} mode=${actual} rollback=${rollback}" "${mode_file}"
  fi
}

status_assert_domain_mode_cleared() {
  local check_id="$1"
  local domain="$2"
  local mode_file="${OUTPUT_DIR}/${check_id}.json"
  local http_status
  http_status="$(control_request "GET" "/api/v1/control/route-split/status" "" "${mode_file}")"
  if [[ "${http_status}" != "200" ]]; then
    record_check "${check_id}" "fail" "status request failed (http=${http_status})" "${mode_file}"
    return
  fi

  local actual
  actual="$(jq -r --arg domain "${domain}" '.data.htmx_domain_overrides[$domain] // "<none>"' "${mode_file}")"
  if [[ "${actual}" == "<none>" ]]; then
    record_check "${check_id}" "pass" "domain=${domain} override cleared" "${mode_file}"
  else
    record_check "${check_id}" "fail" "domain=${domain} expected=<none> actual=${actual}" "${mode_file}"
  fi
}

apply_override() {
  local check_id="$1"
  local target="$2"
  local domain="$3"
  local output_file="${OUTPUT_DIR}/${check_id}.json"
  local payload
  payload="$(jq -nc --arg target "${target}" --arg domain "${domain}" '{target:$target,domain:$domain}')"
  local http_status
  http_status="$(control_request "POST" "/api/v1/control/route-split/override" "${payload}" "${output_file}")"
  if [[ "${http_status}" == "200" ]]; then
    record_check "${check_id}" "pass" "target=${target} domain=${domain}" "${output_file}"
  else
    record_check "${check_id}" "fail" "target=${target} domain=${domain} http=${http_status}" "${output_file}"
  fi
}

IFS=',' read -r -a domains <<<"${DOMAINS_CSV}"

status_snapshot "pre" || true

for domain in "${domains[@]}"; do
  trimmed_domain="$(echo "${domain}" | xargs)"
  if [[ -z "${trimmed_domain}" ]]; then
    continue
  fi

  apply_override "apply-${trimmed_domain}-full-page" "htmx_full_page" "${trimmed_domain}"
  status_assert_domain_mode "assert-${trimmed_domain}-full-page" "${trimmed_domain}" "full_page"

  apply_override "apply-${trimmed_domain}-rollback" "htmx_rollback" "${trimmed_domain}"
  status_assert_domain_mode_matches_rollback "assert-${trimmed_domain}-rollback" "${trimmed_domain}"

  apply_override "apply-${trimmed_domain}-clear" "htmx_clear" "${trimmed_domain}"
  status_assert_domain_mode_cleared "assert-${trimmed_domain}-clear" "${trimmed_domain}"
done

status_snapshot "post" || true

jq -s \
  --arg generated_at "${TIMESTAMP}" \
  --arg base_url "${BASE_URL}" \
  --arg domains "${DOMAINS_CSV}" \
  '
  def count_status(s): map(select(.status == s)) | length;
  {
    schema: "openagents.htmx.route_group_canary.v1",
    generated_at: $generated_at,
    base_url: $base_url,
    domains: ($domains | split(",") | map(gsub("^\\s+|\\s+$"; "")) | map(select(. != ""))),
    totals: {
      check_count: length,
      passed: count_status("pass"),
      failed: count_status("fail")
    },
    overall_status: (if count_status("fail") > 0 then "failed" else "passed" end),
    checks: .
  }
  ' "${RESULTS_JSONL}" >"${SUMMARY_JSON}"

{
  echo "# HTMX Route-Group Canary"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Base URL: ${BASE_URL}"
  echo "- Domains: ${DOMAINS_CSV}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Check | Status | Detail |"
  echo "| --- | --- | --- |"
  jq -r '.checks[] | "| \(.check_id) | \(.status) | \(.detail) |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[htmx-route-group-canary] summary: ${SUMMARY_JSON}"
echo "[htmx-route-group-canary] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: HTMX route-group canary failed" >&2
  exit 1
fi
