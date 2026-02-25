#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/legacy-serving-retirement/${TIMESTAMP}}"
RESULTS_JSONL="${OUTPUT_DIR}/checks.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

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

  echo "[legacy-serving-retired] ${check_id}: ${status}"
  if [[ "${status}" == "fail" ]]; then
    overall_failed=1
  fi
}

check_exists() {
  local check_id="$1"
  local path="$2"
  if [[ -e "${ROOT_DIR}/${path}" ]]; then
    record_check "${check_id}" "pass" "exists: ${path}" "${path}"
  else
    record_check "${check_id}" "fail" "missing required path: ${path}" "${path}"
  fi
}

check_file_contains() {
  local check_id="$1"
  local path="$2"
  local pattern="$3"
  if rg -q --fixed-strings "${pattern}" "${ROOT_DIR}/${path}"; then
    record_check "${check_id}" "pass" "pattern found in ${path}: ${pattern}" "${path}"
  else
    record_check "${check_id}" "fail" "pattern missing in ${path}: ${pattern}" "${path}"
  fi
}

check_file_not_contains() {
  local check_id="$1"
  local path="$2"
  local pattern="$3"
  if rg -q --fixed-strings "${pattern}" "${ROOT_DIR}/${path}"; then
    record_check "${check_id}" "fail" "unexpected pattern in ${path}: ${pattern}" "${path}"
  else
    record_check "${check_id}" "pass" "pattern absent in ${path}: ${pattern}" "${path}"
  fi
}

check_absent_pattern_in_paths() {
  local check_id="$1"
  local pattern="$2"
  shift 2
  local output_file="${OUTPUT_DIR}/${check_id}.txt"

  if rg -n -e "${pattern}" "$@" >"${output_file}"; then
    record_check "${check_id}" "fail" "forbidden pattern found: ${pattern}" "${output_file}"
  else
    record_check "${check_id}" "pass" "forbidden pattern absent: ${pattern}" "${output_file}"
  fi
}

check_exists "rust-deploy-script" "apps/openagents.com/service/deploy/deploy-production.sh"
check_exists "rust-deploy-staging-script" "apps/openagents.com/service/deploy/deploy-staging.sh"
check_exists "rust-service-readme" "apps/openagents.com/README.md"
check_exists "legacy-archive-dir" "apps/openagents.com/deploy/archived-laravel"
check_exists "legacy-agents-archive" "apps/openagents.com/docs/archived/legacy-laravel-deploy/AGENTS.laravel-boost.md"
check_exists "rust-agents-active" "apps/openagents.com/AGENTS.md"

check_file_contains "wrapper-forwards-rust" "apps/openagents.com/deploy/deploy-production.sh" "Forwarding to apps/openagents.com/service/deploy/deploy-production.sh."
check_file_contains "deploy-readme-rust-only" "apps/openagents.com/deploy/README.md" "Deploy Entry Points (Rust-Only Active Lane)"
check_file_contains "app-readme-legacy-archived" "apps/openagents.com/README.md" "historical/transition artifacts"
check_file_contains "project-overview-archive-note" "docs/core/PROJECT_OVERVIEW.md" "apps/openagents.com/app/"

check_file_not_contains "active-agents-no-laravel-boost" "apps/openagents.com/AGENTS.md" "Laravel Boost"

check_absent_pattern_in_paths \
  "service-deploy-no-php-lane" \
  "php artisan|laravel-vite-plugin|npm run dev|mix test|composer install|composer update" \
  "${ROOT_DIR}/apps/openagents.com/service/deploy" \
  "${ROOT_DIR}/apps/openagents.com/service/docs"

expected_top_level="${OUTPUT_DIR}/expected-deploy-top-level.txt"
actual_top_level="${OUTPUT_DIR}/actual-deploy-top-level.txt"
cat >"${expected_top_level}" <<'LIST'
apps/openagents.com/deploy/deploy-production.sh
apps/openagents.com/deploy/README.md
LIST
find "${ROOT_DIR}/apps/openagents.com/deploy" -maxdepth 1 -type f \
  | sed "s|${ROOT_DIR}/||" \
  | sort >"${actual_top_level}"
if diff -u "${expected_top_level}" "${actual_top_level}" >"${OUTPUT_DIR}/deploy-top-level.diff"; then
  record_check "deploy-top-level-files" "pass" "top-level deploy files are rust wrapper + readme only" "${actual_top_level}"
else
  record_check "deploy-top-level-files" "fail" "unexpected top-level deploy files present" "${OUTPUT_DIR}/deploy-top-level.diff"
fi

jq -s \
  --arg generated_at "${TIMESTAMP}" \
  '{
    schema: "openagents.webparity.legacy_serving_retired.v1",
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
  echo "# Laravel Serving Retirement Verification"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Check | Status | Detail | Artifact |"
  echo "| --- | --- | --- | --- |"
  jq -r '.checks[] | "| \(.check_id) | \(.status) | \(.detail) | `\(.artifact // "")` |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[legacy-serving-retired] summary: ${SUMMARY_JSON}"
echo "[legacy-serving-retired] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: laravel serving retirement verification failed" >&2
  exit 1
fi
