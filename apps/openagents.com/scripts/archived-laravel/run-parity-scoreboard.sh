#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
APP_DIR="${ROOT_DIR}/apps/openagents.com"
CONFIG_PATH="${APP_DIR}/docs/parity-manifests/scoreboard-domains.json"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${APP_DIR}/storage/app/parity-scoreboard/${TIMESTAMP}}"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"
RESULTS_JSONL="${OUTPUT_DIR}/domain-results.jsonl"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT

require_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "error: missing required command: ${command_name}" >&2
    exit 1
  fi
}

require_command php
require_command jq
require_command diff
require_command sha256sum

if [[ ! -f "${CONFIG_PATH}" ]]; then
  echo "error: scoreboard config missing: ${CONFIG_PATH}" >&2
  exit 1
fi

mkdir -p "${OUTPUT_DIR}"
: >"${RESULTS_JSONL}"

MANIFEST_CAPTURE_DIR="${TMP_DIR}/manifests"
FIXTURE_CAPTURE_DIR="${TMP_DIR}/fixtures"
mkdir -p "${MANIFEST_CAPTURE_DIR}" "${FIXTURE_CAPTURE_DIR}" "${OUTPUT_DIR}/diffs"

(
  cd "${APP_DIR}"
  php artisan ops:export-parity-manifests --output="${MANIFEST_CAPTURE_DIR}" >/dev/null
  php artisan ops:capture-parity-contract-fixtures \
    --output="${FIXTURE_CAPTURE_DIR}" \
    --openapi-temp=storage/app/openapi-parity-capture.json >/dev/null
)

canonicalize_json() {
  local source_path="$1"
  local target_path="$2"
  jq -S '
    del(.. | .generated_at?) |
    del(.. | .generatedAt?)
  ' "${source_path}" >"${target_path}"
}

regression_count=0
drift_count=0

while IFS=$'\t' read -r domain_id migrated owner baseline_rel captured_rel; do
  baseline_abs="${APP_DIR}/${baseline_rel}"
  captured_abs="${TMP_DIR}/${captured_rel}"
  canonical_baseline="${TMP_DIR}/${domain_id}.baseline.canonical.json"
  canonical_captured="${TMP_DIR}/${domain_id}.captured.canonical.json"
  diff_path="${OUTPUT_DIR}/diffs/${domain_id}.diff"

  status="pass"
  reason=""

  if [[ ! -f "${baseline_abs}" ]]; then
    status="fail"
    reason="missing_baseline"
    : >"${diff_path}"
  elif [[ ! -f "${captured_abs}" ]]; then
    status="fail"
    reason="missing_capture"
    : >"${diff_path}"
  else
    canonicalize_json "${baseline_abs}" "${canonical_baseline}"
    canonicalize_json "${captured_abs}" "${canonical_captured}"

    if diff -u "${canonical_baseline}" "${canonical_captured}" >"${diff_path}"; then
      rm -f "${diff_path}"
      status="pass"
    else
      if [[ "${migrated}" == "true" ]]; then
        status="fail"
        reason="contract_regression"
      else
        status="drift"
        reason="baseline_drift"
      fi
    fi
  fi

  if [[ "${status}" == "fail" ]]; then
    regression_count=$((regression_count + 1))
  elif [[ "${status}" == "drift" ]]; then
    drift_count=$((drift_count + 1))
  fi

  baseline_hash=""
  captured_hash=""
  if [[ -f "${baseline_abs}" ]]; then
    baseline_hash="$(sha256sum "${baseline_abs}" | awk '{print $1}')"
  fi
  if [[ -f "${captured_abs}" ]]; then
    captured_hash="$(sha256sum "${captured_abs}" | awk '{print $1}')"
  fi

  jq -n \
    --arg id "${domain_id}" \
    --arg owner "${owner}" \
    --arg status "${status}" \
    --arg reason "${reason}" \
    --arg migrated "${migrated}" \
    --arg baseline_path "${baseline_rel}" \
    --arg captured_path "${captured_rel}" \
    --arg baseline_hash "${baseline_hash}" \
    --arg captured_hash "${captured_hash}" \
    --arg diff_path "$( [[ -f "${diff_path}" ]] && echo "${diff_path}" || echo "" )" \
    '{
      domain_id: $id,
      owner: $owner,
      status: $status,
      reason: (if $reason == "" then null else $reason end),
      migrated: ($migrated == "true"),
      baseline_path: $baseline_path,
      captured_path: $captured_path,
      baseline_hash: (if $baseline_hash == "" then null else $baseline_hash end),
      captured_hash: (if $captured_hash == "" then null else $captured_hash end),
      diff_path: (if $diff_path == "" then null else $diff_path end)
    }' >>"${RESULTS_JSONL}"
done < <(jq -r '.domains[] | [.id, (.migrated|tostring), .owner, .baseline_path, .captured_path] | @tsv' "${CONFIG_PATH}")

jq -s \
  --slurpfile config "${CONFIG_PATH}" \
  --arg generated_at "${TIMESTAMP}" \
  '
  def count_status(s): map(select(.status == s)) | length;
  {
    schema: "openagents.webparity.scoreboard.v1",
    generated_at: $generated_at,
    config: $config[0],
    totals: {
      domain_count: length,
      passed: count_status("pass"),
      drift: count_status("drift"),
      failed: count_status("fail")
    },
    overall_status: (
      if count_status("fail") > 0 then "failed"
      elif count_status("drift") > 0 then "drift"
      else "passed"
      end
    ),
    domains: .
  }
  ' "${RESULTS_JSONL}" >"${SUMMARY_JSON}"

{
  echo "# Web Parity Scoreboard"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.drift' "${SUMMARY_JSON}") drift / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Domain | Owner | Migrated | Status | Reason | Diff |"
  echo "| --- | --- | --- | --- | --- | --- |"
  jq -r '.domains[] | "| \(.domain_id) | \(.owner) | \(.migrated) | \(.status) | \(.reason // "") | \((.diff_path // "") | if . == "" then "" else "`" + . + "`" end) |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[web-parity-scoreboard] summary: ${SUMMARY_JSON}"
echo "[web-parity-scoreboard] report: ${SUMMARY_MD}"

if [[ "${regression_count}" -gt 0 ]]; then
  echo "error: detected ${regression_count} migrated-domain contract regression(s)." >&2
  exit 1
fi

if [[ "${drift_count}" -gt 0 ]]; then
  echo "warning: detected ${drift_count} drifting domain(s) (non-migrated)." >&2
fi
