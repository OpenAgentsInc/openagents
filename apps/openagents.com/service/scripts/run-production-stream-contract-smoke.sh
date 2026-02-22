#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/production-stream-smoke/${TIMESTAMP}}"
RESULTS_JSONL="${OUTPUT_DIR}/checks.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

BASE_URL="${BASE_URL:-https://openagents.com}"
BASE_URL="${BASE_URL%/}"
AUTH_TOKEN="${AUTH_TOKEN:-}"
DRY_RUN="${DRY_RUN:-0}"
PHASE="${PHASE:-runtime}"
CURL_TIMEOUT_SECONDS="${CURL_TIMEOUT_SECONDS:-20}"
COMPAT_CLIENT_BUILD_ID="${COMPAT_CLIENT_BUILD_ID:-20260221T130000Z}"
COMPAT_PROTOCOL_VERSION="${COMPAT_PROTOCOL_VERSION:-openagents.control.v1}"
COMPAT_SCHEMA_VERSION="${COMPAT_SCHEMA_VERSION:-1}"
THREAD_ID_PREFIX="${THREAD_ID_PREFIX:-thread_prod_stream_smoke}"

require_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "error: missing required command: ${command_name}" >&2
    exit 1
  fi
}

require_command jq
if [[ "${DRY_RUN}" != "1" ]]; then
  require_command curl
fi

if [[ "${DRY_RUN}" != "0" && "${DRY_RUN}" != "1" ]]; then
  echo "error: DRY_RUN must be 0 or 1" >&2
  exit 1
fi

if [[ "${DRY_RUN}" == "0" && -z "${AUTH_TOKEN}" ]]; then
  echo "error: AUTH_TOKEN is required when DRY_RUN=0" >&2
  exit 1
fi

mkdir -p "${OUTPUT_DIR}"
: >"${RESULTS_JSONL}"

overall_failed=0

record_check() {
  local check_id="$1"
  local status="$2"
  local endpoint="$3"
  local detail="$4"
  local http_status="${5:-}"
  local headers_path="${6:-}"
  local body_path="${7:-}"

  jq -n \
    --arg check_id "${check_id}" \
    --arg status "${status}" \
    --arg endpoint "${endpoint}" \
    --arg detail "${detail}" \
    --arg http_status "${http_status}" \
    --arg headers_path "${headers_path}" \
    --arg body_path "${body_path}" \
    '{
      check_id: $check_id,
      status: $status,
      endpoint: $endpoint,
      detail: $detail,
      http_status: (if $http_status == "" then null else $http_status end),
      headers_path: (if $headers_path == "" then null else $headers_path end),
      body_path: (if $body_path == "" then null else $body_path end),
      recorded_at: (now | todateiso8601)
    }' >>"${RESULTS_JSONL}"

  echo "[stream-smoke] ${check_id}: ${status}"
  if [[ "${status}" == "fail" ]]; then
    overall_failed=1
  fi
}

write_dry_run_response() {
  local headers_path="$1"
  local body_path="$2"

  cat >"${headers_path}" <<'EOF_HEADERS'
HTTP/1.1 200 OK
content-type: text/event-stream; charset=utf-8
x-vercel-ai-ui-message-stream: v1
cache-control: no-store
EOF_HEADERS

  cat >"${body_path}" <<'EOF_BODY'
data: {"type":"start","threadId":"thread_smoke"}

data: {"type":"start-step","threadId":"thread_smoke","turnId":"turn_smoke","model":"gpt-5.2-codex"}

data: {"type":"finish-step","turnId":"turn_smoke","status":"accepted"}

data: {"type":"finish","status":"accepted"}

data: [DONE]

EOF_BODY
}

probe_stream_endpoint() {
  local check_id="$1"
  local endpoint="$2"
  local payload="$3"
  local headers_path="${OUTPUT_DIR}/${check_id}.headers"
  local body_path="${OUTPUT_DIR}/${check_id}.body"
  local http_status="200"

  if [[ "${DRY_RUN}" == "1" ]]; then
    write_dry_run_response "${headers_path}" "${body_path}"
  else
    http_status="$(curl -sS --max-time "${CURL_TIMEOUT_SECONDS}" \
      -D "${headers_path}" \
      -o "${body_path}" \
      -w "%{http_code}" \
      -X POST \
      "${BASE_URL}${endpoint}" \
      -H "authorization: Bearer ${AUTH_TOKEN}" \
      -H "content-type: application/json" \
      -H "x-oa-client-build-id: ${COMPAT_CLIENT_BUILD_ID}" \
      -H "x-oa-protocol-version: ${COMPAT_PROTOCOL_VERSION}" \
      -H "x-oa-schema-version: ${COMPAT_SCHEMA_VERSION}" \
      --data "${payload}")"
  fi

  local normalized_headers
  normalized_headers="$(tr '[:upper:]' '[:lower:]' <"${headers_path}")"

  local has_content_type=0
  local has_vercel_header=0
  local has_retired_header=0
  local has_start=0
  local has_start_step=0
  local has_finish_step=0
  local has_finish=0
  local done_count=0
  local detail=()

  if grep -q "^content-type: text/event-stream; charset=utf-8" <<<"${normalized_headers}"; then
    has_content_type=1
  fi

  if grep -q "^x-vercel-ai-ui-message-stream: v1" <<<"${normalized_headers}"; then
    has_vercel_header=1
  fi

  if grep -q "^x-oa-legacy-chat-retired:" <<<"${normalized_headers}"; then
    has_retired_header=1
  fi

  if grep -q '"type":"start"' "${body_path}"; then
    has_start=1
  fi
  if grep -q '"type":"start-step"' "${body_path}"; then
    has_start_step=1
  fi
  if grep -q '"type":"finish-step"' "${body_path}"; then
    has_finish_step=1
  fi
  if grep -q '"type":"finish"' "${body_path}"; then
    has_finish=1
  fi
  done_count="$(grep -Ec '^data: \[DONE\]$' "${body_path}" || true)"

  [[ "${http_status}" == "200" ]] || detail+=("status=${http_status}")
  [[ "${has_content_type}" == "1" ]] || detail+=("missing-content-type")
  [[ "${has_vercel_header}" == "1" ]] || detail+=("missing-vercel-header")
  [[ "${has_retired_header}" == "0" ]] || detail+=("unexpected-retired-header")
  [[ "${has_start}" == "1" ]] || detail+=("missing-start")
  [[ "${has_start_step}" == "1" ]] || detail+=("missing-start-step")
  [[ "${has_finish_step}" == "1" ]] || detail+=("missing-finish-step")
  [[ "${has_finish}" == "1" ]] || detail+=("missing-finish")
  [[ "${done_count}" == "1" ]] || detail+=("done-count=${done_count}")

  if [[ "${#detail[@]}" -eq 0 ]]; then
    record_check "${check_id}" "pass" "${endpoint}" "stream contract verified" "${http_status}" "${headers_path}" "${body_path}"
  else
    record_check "${check_id}" "fail" "${endpoint}" "$(IFS=,; echo "${detail[*]}")" "${http_status}" "${headers_path}" "${body_path}"
  fi
}

thread_id="$(printf '%s_%s' "${THREAD_ID_PREFIX}" "${TIMESTAMP}" | tr '[:upper:]' '[:lower:]')"

probe_stream_endpoint \
  "${PHASE}-chat-stream" \
  "/api/chat/stream" \
  "$(jq -nc --arg thread_id "${thread_id}" --arg text "production stream smoke ${PHASE}" '{id:$thread_id,messages:[{role:"user",content:$text}]}')"

probe_stream_endpoint \
  "${PHASE}-chats-stream" \
  "/api/chats/${thread_id}/stream" \
  "$(jq -nc --arg text "production stream smoke path ${PHASE}" '{messages:[{role:"user",content:[{type:"text",text:$text}]}]}')"

jq -s --arg generated_at "${TIMESTAMP}" --arg base_url "${BASE_URL}" --arg phase "${PHASE}" --argjson dry_run "$( [[ "${DRY_RUN}" == "1" ]] && echo true || echo false )" '
  def count_status(s): map(select(.status == s)) | length;
  {
    schema: "openagents.webparity.production_stream_smoke.v1",
    generated_at: $generated_at,
    base_url: $base_url,
    phase: $phase,
    dry_run: $dry_run,
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
  echo "# Production Stream Contract Smoke"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Base URL: ${BASE_URL}"
  echo "- Phase: ${PHASE}"
  echo "- Dry run: ${DRY_RUN}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Check | Status | Endpoint | Detail | HTTP |"
  echo "| --- | --- | --- | --- | --- |"
  jq -r '.checks[] | "| \(.check_id) | \(.status) | `\(.endpoint)` | \(.detail) | \(.http_status // "") |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[stream-smoke] summary: ${SUMMARY_JSON}"
echo "[stream-smoke] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: production stream smoke contract checks failed" >&2
  exit 1
fi
