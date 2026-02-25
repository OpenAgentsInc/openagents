#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/production-route-flip/${TIMESTAMP}}"
RESULTS_JSONL="${OUTPUT_DIR}/checks.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

BASE_URL="${BASE_URL:-https://openagents.com}"
BASE_URL="${BASE_URL%/}"
CONTROL_ACCESS_TOKEN="${CONTROL_ACCESS_TOKEN:-}"
APPLY="${APPLY:-0}"
COHORT_KEY="${COHORT_KEY:-prod-rust-route-flip}"
CURL_TIMEOUT_SECONDS="${CURL_TIMEOUT_SECONDS:-15}"

if [[ -z "${CONTROL_ACCESS_TOKEN}" ]]; then
  echo "error: CONTROL_ACCESS_TOKEN is required" >&2
  exit 1
fi

if [[ "${APPLY}" != "0" && "${APPLY}" != "1" ]]; then
  echo "error: APPLY must be 0 or 1" >&2
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
  local kind="$3"
  local path="$4"
  local detail="$5"
  local http_status="${6:-}"
  local artifact="${7:-}"

  jq -n \
    --arg check_id "${check_id}" \
    --arg status "${status}" \
    --arg kind "${kind}" \
    --arg path "${path}" \
    --arg detail "${detail}" \
    --arg http_status "${http_status}" \
    --arg artifact "${artifact}" \
    '{
      check_id: $check_id,
      status: $status,
      kind: $kind,
      path: $path,
      detail: $detail,
      http_status: (if $http_status == "" then null else $http_status end),
      artifact: (if $artifact == "" then null else $artifact end),
      recorded_at: (now | todateiso8601)
    }' >>"${RESULTS_JSONL}"

  echo "[rust-route-flip] ${check_id}: ${status}"
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

pre_status_file="${OUTPUT_DIR}/status-pre.json"
pre_status_http="$(control_request "GET" "/api/v1/control/route-split/status" "" "${pre_status_file}")"
if [[ "${pre_status_http}" == "200" ]]; then
  record_check "status-pre" "pass" "control_status" "/api/v1/control/route-split/status" "status snapshot captured" "${pre_status_http}" "${pre_status_file}"
else
  record_check "status-pre" "fail" "control_status" "/api/v1/control/route-split/status" "status endpoint failed" "${pre_status_http}" "${pre_status_file}"
fi

if [[ "${APPLY}" == "1" ]]; then
  clear_payload='{"target":"clear"}'
  clear_file="${OUTPUT_DIR}/apply-clear-global.json"
  clear_http="$(control_request "POST" "/api/v1/control/route-split/override" "${clear_payload}" "${clear_file}")"
  if [[ "${clear_http}" == "200" ]]; then
    record_check "apply-clear-global" "pass" "apply" "/api/v1/control/route-split/override" "global override cleared" "${clear_http}" "${clear_file}"
  else
    record_check "apply-clear-global" "fail" "apply" "/api/v1/control/route-split/override" "failed to clear global override" "${clear_http}" "${clear_file}"
  fi

  for domain in auth_entry account_settings_admin billing_l402 chat_pilot; do
    payload="$(jq -nc --arg target "rust" --arg domain "${domain}" '{target:$target,domain:$domain}')"
    output_file="${OUTPUT_DIR}/apply-domain-${domain}.json"
    http_status="$(control_request "POST" "/api/v1/control/route-split/override" "${payload}" "${output_file}")"
    if [[ "${http_status}" == "200" ]]; then
      record_check "apply-domain-${domain}" "pass" "apply" "/api/v1/control/route-split/override" "set domain override to rust" "${http_status}" "${output_file}"
    else
      record_check "apply-domain-${domain}" "fail" "apply" "/api/v1/control/route-split/override" "failed to set domain override to rust" "${http_status}" "${output_file}"
    fi
  done

  global_payload='{"target":"rust"}'
  global_file="${OUTPUT_DIR}/apply-global-rust.json"
  global_http="$(control_request "POST" "/api/v1/control/route-split/override" "${global_payload}" "${global_file}")"
  if [[ "${global_http}" == "200" ]]; then
    record_check "apply-global-rust" "pass" "apply" "/api/v1/control/route-split/override" "set global override to rust" "${global_http}" "${global_file}"
  else
    record_check "apply-global-rust" "fail" "apply" "/api/v1/control/route-split/override" "failed to set global override to rust" "${global_http}" "${global_file}"
  fi
else
  record_check "apply-skipped" "skip" "apply" "/api/v1/control/route-split/override" "APPLY=0; no control-plane mutations executed"
fi

post_status_file="${OUTPUT_DIR}/status-post.json"
post_status_http="$(control_request "GET" "/api/v1/control/route-split/status" "" "${post_status_file}")"
if [[ "${post_status_http}" == "200" ]]; then
  record_check "status-post" "pass" "control_status" "/api/v1/control/route-split/status" "status snapshot captured" "${post_status_http}" "${post_status_file}"
else
  record_check "status-post" "fail" "control_status" "/api/v1/control/route-split/status" "status endpoint failed" "${post_status_http}" "${post_status_file}"
fi

for path in "/" "/feed" "/login" "/settings/profile" "/l402/paywalls" "/admin" "/api/auth/email" "/api/runtime/tools/execute" "/api/runtime/codex/workers"; do
  slug="$(printf '%s' "${path}" | tr '/:{}' '_' | tr -s '_' | sed 's/^_//;s/_$//')"
  if [[ -z "${slug}" ]]; then
    slug="root"
  fi

  payload="$(jq -nc --arg path "${path}" --arg cohort_key "${COHORT_KEY}" '{path:$path,cohort_key:$cohort_key}')"
  eval_file="${OUTPUT_DIR}/evaluate-${slug}.json"
  eval_http="$(control_request "POST" "/api/v1/control/route-split/evaluate" "${payload}" "${eval_file}")"

  if [[ "${eval_http}" != "200" ]]; then
    record_check "evaluate-${slug}" "fail" "route_evaluate" "${path}" "evaluate request failed" "${eval_http}" "${eval_file}"
    continue
  fi

  target="$(jq -r '.data.target // empty' "${eval_file}")"
  reason="$(jq -r '.data.reason // empty' "${eval_file}")"
  if [[ "${target}" == "rust_shell" ]]; then
    record_check "evaluate-${slug}" "pass" "route_evaluate" "${path}" "target=${target}, reason=${reason}" "${eval_http}" "${eval_file}"
  else
    record_check "evaluate-${slug}" "fail" "route_evaluate" "${path}" "expected rust_shell, got ${target:-<empty>} (reason=${reason:-<empty>})" "${eval_http}" "${eval_file}"
  fi
done

probe_write_path() {
  local probe_id="$1"
  local method="$2"
  local path="$3"
  local payload="$4"

  local headers_file="${OUTPUT_DIR}/probe-${probe_id}.headers"
  local body_file="${OUTPUT_DIR}/probe-${probe_id}.json"

  local http_status
  http_status="$(curl -sS --max-time "${CURL_TIMEOUT_SECONDS}" \
    -D "${headers_file}" \
    -o "${body_file}" \
    -w "%{http_code}" \
    -X "${method}" \
    "${BASE_URL}${path}" \
    -H "content-type: application/json" \
    --data "${payload}")"

  local location
  location="$(awk 'tolower($1)=="location:" {print $2}' "${headers_file}" | tr -d '\r' | head -n 1 || true)"

  if [[ "${http_status}" =~ ^30[1278]$ ]] || [[ -n "${location}" ]]; then
    record_check "${probe_id}" "fail" "write_probe" "${path}" "write path redirected (status=${http_status}, location=${location:-<none>})" "${http_status}" "${headers_file}"
  else
    record_check "${probe_id}" "pass" "write_probe" "${path}" "write path handled by rust endpoint (status=${http_status})" "${http_status}" "${headers_file}"
  fi
}

probe_write_path "write-probe-auth-email" "POST" "/api/auth/email" '{}'
probe_write_path "write-probe-settings-profile" "PATCH" "/api/settings/profile" '{}'
probe_write_path "write-probe-runtime-tools" "POST" "/api/runtime/tools/execute" '{}'

apply_bool="false"
if [[ "${APPLY}" == "1" ]]; then
  apply_bool="true"
fi

jq -s \
  --arg generated_at "${TIMESTAMP}" \
  --arg base_url "${BASE_URL}" \
  --arg cohort_key "${COHORT_KEY}" \
  --argjson apply "${apply_bool}" \
  '
  def count_status(s): map(select(.status == s)) | length;
  {
    schema: "openagents.webparity.route_flip.v1",
    generated_at: $generated_at,
    base_url: $base_url,
    cohort_key: $cohort_key,
    apply: $apply,
    totals: {
      check_count: length,
      passed: count_status("pass"),
      failed: count_status("fail"),
      skipped: count_status("skip")
    },
    overall_status: (if count_status("fail") > 0 then "failed" else "passed" end),
    checks: .
  }
  ' "${RESULTS_JSONL}" >"${SUMMARY_JSON}"

{
  echo "# Production Rust Route Flip"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Base URL: ${BASE_URL}"
  echo "- Apply mode: ${APPLY}"
  echo "- Cohort key: ${COHORT_KEY}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail / $(jq -r '.totals.skipped' "${SUMMARY_JSON}") skipped"
  echo
  echo "| Check | Status | Kind | Path | Detail | HTTP |"
  echo "| --- | --- | --- | --- | --- | --- |"
  jq -r '.checks[] | "| \(.check_id) | \(.status) | \(.kind) | `\(.path)` | \(.detail) | \(.http_status // "") |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[rust-route-flip] summary: ${SUMMARY_JSON}"
echo "[rust-route-flip] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: rust route flip verification failed" >&2
  exit 1
fi
