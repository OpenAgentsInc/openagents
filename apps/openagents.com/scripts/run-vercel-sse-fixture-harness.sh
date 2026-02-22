#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/vercel-sse-fixture-harness/${TIMESTAMP}}"
RESULTS_JSONL="${OUTPUT_DIR}/checks.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

FIXTURE_DIR="${ROOT_DIR}/apps/openagents.com/docs/parity-fixtures/vercel-sse-compat-v1"
MAP_FILTER="${ROOT_DIR}/apps/openagents.com/scripts/vercel-sse-fixture-map.jq"
INPUT_SCENARIOS="${FIXTURE_DIR}/codex-event-scenarios.json"
GOLDEN_TRANSCRIPTS="${FIXTURE_DIR}/golden-sse-transcripts.json"
ERROR_FIXTURES="${FIXTURE_DIR}/golden-error-fixtures.json"
FIXTURE_INDEX="${FIXTURE_DIR}/fixture-index.json"
GENERATED_TRANSCRIPTS="${OUTPUT_DIR}/generated-sse-transcripts.json"

mkdir -p "${OUTPUT_DIR}"
: >"${RESULTS_JSONL}"

overall_failed=0

require_command() {
  local command_name="$1"
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    echo "error: missing required command: ${command_name}" >&2
    exit 1
  fi
}

require_command jq
require_command diff

check_source_paths() {
  local missing=0
  while IFS= read -r path; do
    if [[ -z "${path}" ]]; then
      continue
    fi
    if [[ ! -e "${ROOT_DIR}/${path}" ]]; then
      echo "missing source path: ${path}" >&2
      missing=1
    fi
  done < <(jq -r '.sources[].path' "${FIXTURE_INDEX}")
  [[ "${missing}" -eq 0 ]]
}

generate_transcripts() {
  jq --arg generated_at "${TIMESTAMP}" \
    -f "${MAP_FILTER}" \
    "${INPUT_SCENARIOS}" >"${GENERATED_TRANSCRIPTS}"
}

compare_transcripts_to_golden() {
  jq -S 'del(.generated_at)' "${GOLDEN_TRANSCRIPTS}" >"${OUTPUT_DIR}/golden.normalized.json"
  jq -S 'del(.generated_at)' "${GENERATED_TRANSCRIPTS}" >"${OUTPUT_DIR}/generated.normalized.json"

  if ! diff -u \
    "${OUTPUT_DIR}/golden.normalized.json" \
    "${OUTPUT_DIR}/generated.normalized.json" >"${OUTPUT_DIR}/drift.diff"; then
    cat "${OUTPUT_DIR}/drift.diff"
    return 1
  fi
}

validate_error_fixture_matrix() {
  jq -e '
    .schema == "openagents.webparity.vercel_sse_error_fixtures.v1"
    and (.errors | type == "array")
    and ((.errors | length) >= 3)
    and any(.errors[]; .id == "invalid_user_text" and .http_status == 422 and .phase == "pre-stream-json")
    and any(.errors[]; .id == "compatibility_upgrade_required" and .http_status == 426 and .phase == "pre-stream-json")
    and any(.errors[]; .id == "terminal_stream_error" and .phase == "in-stream-terminal")
  ' "${ERROR_FIXTURES}" >/dev/null
}

run_check() {
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

  echo "[vercel-sse-fixture-harness] ${check_id}: ${status}"
}

run_check "fixture-source-paths" "Fixture source references exist" check_source_paths
run_check "generate-transcripts" "Regenerate normalized SSE transcripts from Codex event scenarios" generate_transcripts
run_check "compare-goldens" "Detect fixture drift against committed golden transcripts" compare_transcripts_to_golden
run_check "error-matrix" "Validate pre-stream/in-stream error fixture matrix" validate_error_fixture_matrix

jq -s --arg generated_at "${TIMESTAMP}" '
  {
    schema: "openagents.webparity.vercel_sse_fixture_harness.v1",
    generated_at: $generated_at,
    totals: {
      check_count: length,
      passed: (map(select(.status == "pass")) | length),
      failed: (map(select(.status == "fail")) | length)
    },
    overall_status: (if (map(select(.status == "fail")) | length) > 0 then "failed" else "passed" end),
    checks: .
  }
' "${RESULTS_JSONL}" >"${SUMMARY_JSON}"

{
  echo "# Vercel SSE Fixture Harness"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Check | Status | Description | Log |"
  echo "| --- | --- | --- | --- |"
  jq -r '.checks[] | "| \(.check_id) | \(.status) | \(.description) | `\(.log_path)` |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[vercel-sse-fixture-harness] summary: ${SUMMARY_JSON}"
echo "[vercel-sse-fixture-harness] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: vercel SSE fixture harness failed" >&2
  exit 1
fi
