#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
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
MAX_CRITICAL_MISMATCHES="${MAX_CRITICAL_MISMATCHES:-0}"
MAX_TOTAL_MISMATCHES="${MAX_TOTAL_MISMATCHES:-0}"
MAX_STREAM_MISMATCHES="${MAX_STREAM_MISMATCHES:-0}"
MAX_P95_LATENCY_DELTA_MS="${MAX_P95_LATENCY_DELTA_MS:-250}"

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

canonicalize_sse() {
  local source_path="$1"
  local target_path="$2"
  local events_jsonl="${target_path}.events.jsonl"
  local done_seen=0

  : >"${events_jsonl}"
  while IFS= read -r raw_line || [[ -n "${raw_line}" ]]; do
    local line
    line="$(printf '%s' "${raw_line}" | sed 's/\r$//')"
    [[ -z "${line}" ]] && continue
    [[ "${line}" != data:* ]] && continue

    local payload
    payload="${line#data:}"
    payload="$(printf '%s' "${payload}" | sed 's/^[[:space:]]*//')"
    if [[ "${payload}" == "[DONE]" ]]; then
      done_seen=1
      continue
    fi

    if jq -e . <<<"${payload}" >/dev/null 2>&1; then
      jq -c '
        del(.request_id, .requestId, .turn_id, .turnId, .generated_at, .generatedAt) |
        del(.. | .request_id?) |
        del(.. | .requestId?) |
        del(.. | .generated_at?) |
        del(.. | .generatedAt?)
      ' <<<"${payload}" >>"${events_jsonl}"
    fi
  done <"${source_path}"

  jq -s --argjson done_seen "${done_seen}" '
    {
      done: ($done_seen == 1),
      events: .,
      order: map(.type // "unknown"),
      tool_events: map(select((.type // "") | startswith("tool-"))),
      finish_events: map(select((.type // "") == "finish" or (.type // "") == "finish-step")),
      error_events: map(select((.type // "") == "error"))
    }
  ' "${events_jsonl}" >"${target_path}"
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
    -w "%{http_code} %{time_total}"
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

  local curl_metrics http_code time_total duration_ms
  if ! curl_metrics="$(curl "${curl_args[@]}")"; then
    http_code="000"
    time_total="0"
  else
    http_code="$(awk '{print $1}' <<<"${curl_metrics}")"
    time_total="$(awk '{print $2}' <<<"${curl_metrics}")"
  fi
  duration_ms="$(awk "BEGIN {printf \"%.0f\", (${time_total:-0}) * 1000}")"

  if [[ "${response_kind}" == "json" ]]; then
    if jq -e . "${body_path}" >/dev/null 2>&1; then
      canonicalize_json "${body_path}" "${normalized_path}"
    else
      cp "${body_path}" "${normalized_path}"
    fi
  elif [[ "${response_kind}" == "sse" ]]; then
    canonicalize_sse "${body_path}" "${normalized_path}"
  else
    cp "${body_path}" "${normalized_path}"
  fi

  local body_hash
  body_hash="$(sha256sum "${normalized_path}" | awk '{print $1}')"

  jq -n \
    --arg http_code "${http_code}" \
    --arg duration_ms "${duration_ms}" \
    --arg body_path "${body_path}" \
    --arg normalized_path "${normalized_path}" \
    --arg body_hash "${body_hash}" \
    '{
      status_code: ($http_code | tonumber),
      duration_ms: ($duration_ms | tonumber),
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
        duration_ms: $rust_result.duration_ms,
        body_hash: $rust_result.body_hash,
        body_path: $rust_result.body_path,
        normalized_path: $rust_result.normalized_path
      },
      legacy: {
        base_url: $legacy_base,
        status_code: $legacy_result.status_code,
        duration_ms: $legacy_result.duration_ms,
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
  --arg max_critical_mismatches "${MAX_CRITICAL_MISMATCHES}" \
  --arg max_total_mismatches "${MAX_TOTAL_MISMATCHES}" \
  --arg max_stream_mismatches "${MAX_STREAM_MISMATCHES}" \
  --arg max_p95_latency_delta_ms "${MAX_P95_LATENCY_DELTA_MS}" \
  '
  def count_status(s): map(select(.status == s)) | length;
  def abs_value(v): if v < 0 then -v else v end;
  def p95(values):
    if (values | length) == 0 then 0
    else (values | sort | .[(((length - 1) * 0.95) | floor)])
    end;
  def rust_durations: [ .[] | select(.status != "skipped") | .rust.duration_ms ];
  def legacy_durations: [ .[] | select(.status != "skipped") | .legacy.duration_ms ];
  {
    schema: "openagents.webparity.staging_dual_run_diff.v1",
    generated_at: $generated_at,
    rust_base_url: $rust_base_url,
    legacy_base_url: $legacy_base_url,
    requests_file: $requests_file,
    thresholds: {
      max_critical_mismatches: ($max_critical_mismatches | tonumber),
      max_total_mismatches: ($max_total_mismatches | tonumber),
      max_stream_mismatches: ($max_stream_mismatches | tonumber),
      max_p95_latency_delta_ms: ($max_p95_latency_delta_ms | tonumber)
    },
    totals: {
      request_count: length,
      passed: count_status("pass"),
      failed: count_status("fail"),
      skipped: count_status("skipped"),
      critical_failed: map(select(.status == "fail" and .critical == true)) | length,
      stream_failed: map(select(.status == "fail" and .response_kind == "sse")) | length
    },
    latency_ms: {
      rust_p95: p95(rust_durations),
      legacy_p95: p95(legacy_durations),
      p95_delta: abs_value(p95(rust_durations) - p95(legacy_durations))
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
  echo "- Stream mismatches: $(jq -r '.totals.stream_failed' "${SUMMARY_JSON}")"
  echo "- Latency p95 (Rust/Legacy/Delta ms): $(jq -r '.latency_ms.rust_p95' "${SUMMARY_JSON}") / $(jq -r '.latency_ms.legacy_p95' "${SUMMARY_JSON}") / $(jq -r '.latency_ms.p95_delta' "${SUMMARY_JSON}")"
  echo "- Thresholds: critical<=$(jq -r '.thresholds.max_critical_mismatches' "${SUMMARY_JSON}"), total<=$(jq -r '.thresholds.max_total_mismatches' "${SUMMARY_JSON}"), stream<=$(jq -r '.thresholds.max_stream_mismatches' "${SUMMARY_JSON}"), p95-delta<=$(jq -r '.thresholds.max_p95_latency_delta_ms' "${SUMMARY_JSON}")ms"
  echo
  echo "| Request | Method | Path | Critical | Status | Reason | Diff |"
  echo "| --- | --- | --- | --- | --- | --- | --- |"
  jq -r '.requests[] | "| \(.request_id) | \(.method) | \(.path) | \(.critical) | \(.status) | \(.reason // "") | \((.diff_path // "") | if . == "" then "" else "`" + . + "`" end) |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[staging-dual-run] summary: ${SUMMARY_JSON}"
echo "[staging-dual-run] report: ${SUMMARY_MD}"

critical_failed="$(jq -r '.totals.critical_failed' "${SUMMARY_JSON}")"
total_failed="$(jq -r '.totals.failed' "${SUMMARY_JSON}")"
stream_failed="$(jq -r '.totals.stream_failed' "${SUMMARY_JSON}")"
p95_latency_delta_ms="$(jq -r '.latency_ms.p95_delta' "${SUMMARY_JSON}")"

if [[ "${critical_failed}" -gt "${MAX_CRITICAL_MISMATCHES}" ]]; then
  echo "error: critical mismatches ${critical_failed} exceed threshold ${MAX_CRITICAL_MISMATCHES}" >&2
  exit 1
fi

if [[ "${total_failed}" -gt "${MAX_TOTAL_MISMATCHES}" ]]; then
  echo "error: total mismatches ${total_failed} exceed threshold ${MAX_TOTAL_MISMATCHES}" >&2
  exit 1
fi

if [[ "${stream_failed}" -gt "${MAX_STREAM_MISMATCHES}" ]]; then
  echo "error: stream mismatches ${stream_failed} exceed threshold ${MAX_STREAM_MISMATCHES}" >&2
  exit 1
fi

if [[ "${p95_latency_delta_ms}" -gt "${MAX_P95_LATENCY_DELTA_MS}" ]]; then
  echo "error: p95 latency delta ${p95_latency_delta_ms}ms exceeds threshold ${MAX_P95_LATENCY_DELTA_MS}ms" >&2
  exit 1
fi

if [[ "${mismatch_count}" -gt 0 ]]; then
  echo "warning: detected ${mismatch_count} mismatch(es) within configured thresholds" >&2
fi

if [[ "${skip_count}" -gt 0 ]]; then
  echo "warning: skipped ${skip_count} auth-required request(s) because AUTH_TOKEN was not provided" >&2
fi
