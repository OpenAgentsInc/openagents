#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CANARY_SCRIPT="${ROOT_DIR}/apps/openagents.com/deploy/canary-rollout.sh"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/deploy/reports/canary-drill-${TIMESTAMP}}"
RESULTS_JSONL="${OUTPUT_DIR}/step-results.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

PROJECT="${PROJECT:-$(gcloud config get-value project 2>/dev/null || true)}"
REGION="${REGION:-us-central1}"
SERVICE="${SERVICE:-openagents-control-service}"
DRY_RUN="${DRY_RUN:-0}"
CANARY_STEPS="${CANARY_STEPS:-5,25,50,100}"

if [[ $# -ne 2 ]]; then
  echo "Usage: run-canary-rollback-drill.sh <stable-revision> <canary-revision>" >&2
  exit 2
fi

STABLE_REVISION="$1"
CANARY_REVISION="$2"

if [[ ! -x "${CANARY_SCRIPT}" ]]; then
  echo "error: canary rollout script missing or not executable: ${CANARY_SCRIPT}" >&2
  exit 1
fi

mkdir -p "${OUTPUT_DIR}"
: >"${RESULTS_JSONL}"

overall_failed=0

run_step() {
  local step_id="$1"
  local description="$2"
  local command="$3"

  local log_path="${OUTPUT_DIR}/${step_id}.log"
  local started_at ended_at status reason
  started_at="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  if (
    cd "${ROOT_DIR}"
    bash -lc "${command}"
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
    --arg step_id "${step_id}" \
    --arg description "${description}" \
    --arg command "${command}" \
    --arg status "${status}" \
    --arg reason "${reason}" \
    --arg started_at "${started_at}" \
    --arg ended_at "${ended_at}" \
    --arg log_path "${log_path}" \
    '{
      step_id: $step_id,
      description: $description,
      command: $command,
      status: $status,
      reason: (if $reason == "" then null else $reason end),
      started_at: $started_at,
      ended_at: $ended_at,
      log_path: $log_path
    }' >>"${RESULTS_JSONL}"

  echo "[canary-drill] ${step_id}: ${status}"
}

base_env="PROJECT='${PROJECT}' REGION='${REGION}' SERVICE='${SERVICE}' DRY_RUN='${DRY_RUN}'"

run_step "status-pre" "Capture pre-drill traffic status" \
  "${base_env} '${CANARY_SCRIPT}' status"

IFS=',' read -r -a step_values <<<"${CANARY_STEPS}"
for step in "${step_values[@]}"; do
  trimmed_step="$(printf '%s' "${step}" | xargs)"
  if [[ -z "${trimmed_step}" ]]; then
    continue
  fi
  if ! [[ "${trimmed_step}" =~ ^[0-9]+$ ]] || (( trimmed_step < 0 || trimmed_step > 100 )); then
    echo "error: invalid canary step value '${trimmed_step}' in CANARY_STEPS" >&2
    exit 1
  fi

  run_step "traffic-${trimmed_step}" "Set canary traffic to ${trimmed_step}%" \
    "${base_env} '${CANARY_SCRIPT}' set-traffic '${STABLE_REVISION}' '${CANARY_REVISION}' '${trimmed_step}'"
done

run_step "rollback" "Rollback traffic to stable revision" \
  "${base_env} '${CANARY_SCRIPT}' rollback '${STABLE_REVISION}'"

run_step "status-post" "Capture post-drill traffic status" \
  "${base_env} '${CANARY_SCRIPT}' status"

jq -s \
  --arg generated_at "${TIMESTAMP}" \
  --arg project "${PROJECT}" \
  --arg region "${REGION}" \
  --arg service "${SERVICE}" \
  --arg stable_revision "${STABLE_REVISION}" \
  --arg canary_revision "${CANARY_REVISION}" \
  --arg dry_run "${DRY_RUN}" \
  '
  def count_status(s): map(select(.status == s)) | length;
  {
    schema: "openagents.webparity.canary_drill.v1",
    generated_at: $generated_at,
    project: $project,
    region: $region,
    service: $service,
    stable_revision: $stable_revision,
    canary_revision: $canary_revision,
    dry_run: ($dry_run == "1"),
    totals: {
      step_count: length,
      passed: count_status("pass"),
      failed: count_status("fail")
    },
    overall_status: (if count_status("fail") > 0 then "failed" else "passed" end),
    steps: .
  }
  ' "${RESULTS_JSONL}" >"${SUMMARY_JSON}"

{
  echo "# Canary Rollback Drill"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Project: ${PROJECT}"
  echo "- Region: ${REGION}"
  echo "- Service: ${SERVICE}"
  echo "- Stable revision: ${STABLE_REVISION}"
  echo "- Canary revision: ${CANARY_REVISION}"
  echo "- Dry run: ${DRY_RUN}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo
  echo "| Step | Status | Reason | Log |"
  echo "| --- | --- | --- | --- |"
  jq -r '.steps[] | "| \(.step_id) | \(.status) | \(.reason // "") | `\(.log_path)` |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[canary-drill] summary: ${SUMMARY_JSON}"
echo "[canary-drill] report: ${SUMMARY_MD}"

if [[ "${overall_failed}" -ne 0 ]]; then
  echo "error: canary/rollback drill failed" >&2
  exit 1
fi
