#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/apps/openagents.com/storage/app/production-stream-cutover/${TIMESTAMP}}"
RESULTS_JSONL="${OUTPUT_DIR}/steps.jsonl"
SUMMARY_JSON="${OUTPUT_DIR}/summary.json"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

ROUTE_FLIP_SCRIPT="${ROOT_DIR}/apps/openagents.com/service/scripts/run-production-rust-route-flip.sh"
STREAM_SMOKE_SCRIPT="${ROOT_DIR}/apps/openagents.com/service/scripts/run-production-stream-contract-smoke.sh"
CANARY_DRILL_SCRIPT="${ROOT_DIR}/apps/openagents.com/service/deploy/run-canary-rollback-drill.sh"
DUAL_RUN_SCRIPT="${ROOT_DIR}/apps/openagents.com/service/scripts/run-staging-dual-run-shadow-diff.sh"

usage() {
  cat <<'EOF'
Usage:
  run-production-stream-cutover.sh <stable-revision> <canary-revision>

Environment:
  BASE_URL                                 default: https://openagents.com
  CONTROL_ACCESS_TOKEN                     required when DRY_RUN=0
  AUTH_TOKEN                               required when DRY_RUN=0
  APPLY_ROUTE_FLIP                         0 verify-only, 1 apply route flip (default: 0)
  DRY_RUN                                  1 for non-destructive rehearsal (default: 1)
  PROJECT                                  default: openagentsgemini
  REGION                                   default: us-central1
  SERVICE                                  default: openagents-control-service
  RUN_DUAL_RUN                             1 to run rust-vs-legacy diff gate (default: 0)
  LEGACY_BASE_URL                          required when RUN_DUAL_RUN=1
  MAX_ROUTE_FLIP_FAILURES                  default: 0
  MAX_STREAM_SMOKE_FAILURES                default: 0
  MAX_CANARY_DRILL_FAILURES                default: 0
  MAX_DUAL_RUN_FAILURES                    default: 0
  MAX_ERROR_BUDGET_CONSUMED_PERCENT        default: 5
  SLO_ERROR_BUDGET_CONSUMED_PERCENT        default: 0
EOF
}

if [[ $# -ne 2 ]]; then
  usage
  exit 2
fi

STABLE_REVISION="$1"
CANARY_REVISION="$2"

BASE_URL="${BASE_URL:-https://openagents.com}"
BASE_URL="${BASE_URL%/}"
CONTROL_ACCESS_TOKEN="${CONTROL_ACCESS_TOKEN:-}"
AUTH_TOKEN="${AUTH_TOKEN:-}"
APPLY_ROUTE_FLIP="${APPLY_ROUTE_FLIP:-0}"
DRY_RUN="${DRY_RUN:-1}"
PROJECT="${PROJECT:-openagentsgemini}"
REGION="${REGION:-us-central1}"
SERVICE="${SERVICE:-openagents-control-service}"
RUN_DUAL_RUN="${RUN_DUAL_RUN:-0}"
LEGACY_BASE_URL="${LEGACY_BASE_URL:-}"

MAX_ROUTE_FLIP_FAILURES="${MAX_ROUTE_FLIP_FAILURES:-0}"
MAX_STREAM_SMOKE_FAILURES="${MAX_STREAM_SMOKE_FAILURES:-0}"
MAX_CANARY_DRILL_FAILURES="${MAX_CANARY_DRILL_FAILURES:-0}"
MAX_DUAL_RUN_FAILURES="${MAX_DUAL_RUN_FAILURES:-0}"
MAX_ERROR_BUDGET_CONSUMED_PERCENT="${MAX_ERROR_BUDGET_CONSUMED_PERCENT:-5}"
SLO_ERROR_BUDGET_CONSUMED_PERCENT="${SLO_ERROR_BUDGET_CONSUMED_PERCENT:-0}"

for cmd in jq bash; do
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "error: missing required command: ${cmd}" >&2
    exit 1
  fi
done

for binary in "${ROUTE_FLIP_SCRIPT}" "${STREAM_SMOKE_SCRIPT}" "${CANARY_DRILL_SCRIPT}" "${DUAL_RUN_SCRIPT}"; do
  if [[ ! -f "${binary}" ]]; then
    echo "error: missing script: ${binary}" >&2
    exit 1
  fi
done

if [[ "${APPLY_ROUTE_FLIP}" != "0" && "${APPLY_ROUTE_FLIP}" != "1" ]]; then
  echo "error: APPLY_ROUTE_FLIP must be 0 or 1" >&2
  exit 1
fi
if [[ "${DRY_RUN}" != "0" && "${DRY_RUN}" != "1" ]]; then
  echo "error: DRY_RUN must be 0 or 1" >&2
  exit 1
fi
if [[ "${RUN_DUAL_RUN}" != "0" && "${RUN_DUAL_RUN}" != "1" ]]; then
  echo "error: RUN_DUAL_RUN must be 0 or 1" >&2
  exit 1
fi

if [[ "${DRY_RUN}" == "0" ]]; then
  if [[ -z "${CONTROL_ACCESS_TOKEN}" ]]; then
    echo "error: CONTROL_ACCESS_TOKEN is required when DRY_RUN=0" >&2
    exit 1
  fi
  if [[ -z "${AUTH_TOKEN}" ]]; then
    echo "error: AUTH_TOKEN is required when DRY_RUN=0" >&2
    exit 1
  fi
fi

if [[ "${RUN_DUAL_RUN}" == "1" && -z "${LEGACY_BASE_URL}" ]]; then
  echo "error: LEGACY_BASE_URL is required when RUN_DUAL_RUN=1" >&2
  exit 1
fi

mkdir -p "${OUTPUT_DIR}"
: >"${RESULTS_JSONL}"

overall_failed=0
route_flip_failed=0
stream_smoke_failed=0
canary_failed=0
dual_run_failed=0
dual_run_p95_delta=0

record_step() {
  local step_id="$1"
  local status="$2"
  local detail="$3"
  local artifact="$4"
  local failure_count="$5"

  jq -n \
    --arg step_id "${step_id}" \
    --arg status "${status}" \
    --arg detail "${detail}" \
    --arg artifact "${artifact}" \
    --arg failure_count "${failure_count}" \
    '{
      step_id: $step_id,
      status: $status,
      detail: $detail,
      artifact: (if $artifact == "" then null else $artifact end),
      failure_count: ($failure_count | tonumber),
      recorded_at: (now | todateiso8601)
    }' >>"${RESULTS_JSONL}"

  echo "[stream-cutover] ${step_id}: ${status}" >&2
  if [[ "${status}" == "fail" ]]; then
    overall_failed=1
  fi
}

run_external_script() {
  local step_id="$1"
  local command="$2"
  local summary_path="$3"

  local log_path="${OUTPUT_DIR}/${step_id}.log"
  if (
    cd "${ROOT_DIR}"
    bash -lc "${command}"
  ) >"${log_path}" 2>&1; then
    local failed_count=0
    if [[ -f "${summary_path}" ]]; then
      failed_count="$(jq -r '.totals.failed // 0' "${summary_path}" 2>/dev/null || echo 1)"
    fi
    record_step "${step_id}" "pass" "command succeeded" "${summary_path}" "${failed_count}"
    printf '%s' "${failed_count}"
    return 0
  fi

  record_step "${step_id}" "fail" "command failed (see log)" "${log_path}" "1"
  printf '1'
  return 1
}

route_flip_dir="${OUTPUT_DIR}/route-flip"
if [[ "${DRY_RUN}" == "1" ]]; then
  record_step "route-flip" "pass" "dry-run rehearsal: route flip not applied" "" "0"
  route_flip_failed=0
else
  mkdir -p "${route_flip_dir}"
  route_flip_cmd="OUTPUT_DIR='${route_flip_dir}' BASE_URL='${BASE_URL}' APPLY='${APPLY_ROUTE_FLIP}' CONTROL_ACCESS_TOKEN='${CONTROL_ACCESS_TOKEN}' '${ROUTE_FLIP_SCRIPT}'"
  route_flip_failed="$(run_external_script "route-flip" "${route_flip_cmd}" "${route_flip_dir}/summary.json" || true)"
fi

stream_pre_dir="${OUTPUT_DIR}/stream-smoke-pre"
mkdir -p "${stream_pre_dir}"
stream_pre_cmd="OUTPUT_DIR='${stream_pre_dir}' BASE_URL='${BASE_URL}' AUTH_TOKEN='${AUTH_TOKEN}' DRY_RUN='${DRY_RUN}' PHASE='pre-cutover' '${STREAM_SMOKE_SCRIPT}'"
stream_pre_failed="$(run_external_script "stream-smoke-pre" "${stream_pre_cmd}" "${stream_pre_dir}/summary.json" || true)"

stream_post_dir="${OUTPUT_DIR}/stream-smoke-post"
mkdir -p "${stream_post_dir}"
stream_post_cmd="OUTPUT_DIR='${stream_post_dir}' BASE_URL='${BASE_URL}' AUTH_TOKEN='${AUTH_TOKEN}' DRY_RUN='${DRY_RUN}' PHASE='post-cutover' '${STREAM_SMOKE_SCRIPT}'"
stream_post_failed="$(run_external_script "stream-smoke-post" "${stream_post_cmd}" "${stream_post_dir}/summary.json" || true)"

stream_smoke_failed=$((stream_pre_failed + stream_post_failed))

canary_dir="${OUTPUT_DIR}/canary-rollback-drill"
mkdir -p "${canary_dir}"
canary_cmd="OUTPUT_DIR='${canary_dir}' PROJECT='${PROJECT}' REGION='${REGION}' SERVICE='${SERVICE}' DRY_RUN='${DRY_RUN}' '${CANARY_DRILL_SCRIPT}' '${STABLE_REVISION}' '${CANARY_REVISION}'"
canary_failed="$(run_external_script "canary-rollback-drill" "${canary_cmd}" "${canary_dir}/summary.json" || true)"

if [[ "${RUN_DUAL_RUN}" == "1" ]]; then
  dual_run_dir="${OUTPUT_DIR}/dual-run-diff"
  mkdir -p "${dual_run_dir}"
  dual_run_cmd="OUTPUT_DIR='${dual_run_dir}' RUST_BASE_URL='${BASE_URL}' LEGACY_BASE_URL='${LEGACY_BASE_URL}' AUTH_TOKEN='${AUTH_TOKEN}' '${DUAL_RUN_SCRIPT}'"
  dual_run_failed="$(run_external_script "dual-run-diff" "${dual_run_cmd}" "${dual_run_dir}/summary.json" || true)"
  if [[ -f "${dual_run_dir}/summary.json" ]]; then
    dual_run_p95_delta="$(jq -r '.latency_ms.p95_delta // 0' "${dual_run_dir}/summary.json" 2>/dev/null || echo 0)"
  fi
else
  record_step "dual-run-diff" "pass" "dual-run gate skipped (RUN_DUAL_RUN=0)" "" "0"
fi

error_budget_gate_status="pass"
error_budget_detail="within budget"
if awk "BEGIN {exit !(${SLO_ERROR_BUDGET_CONSUMED_PERCENT} <= ${MAX_ERROR_BUDGET_CONSUMED_PERCENT})}"; then
  :
else
  error_budget_gate_status="fail"
  error_budget_detail="error budget exceeded"
  overall_failed=1
fi

record_step \
  "slo-error-budget-gate" \
  "${error_budget_gate_status}" \
  "${error_budget_detail} (${SLO_ERROR_BUDGET_CONSUMED_PERCENT}% <= ${MAX_ERROR_BUDGET_CONSUMED_PERCENT}%)" \
  "" \
  "$( [[ "${error_budget_gate_status}" == "pass" ]] && echo 0 || echo 1 )"

if [[ "${route_flip_failed}" -gt "${MAX_ROUTE_FLIP_FAILURES}" ]]; then
  overall_failed=1
fi
if [[ "${stream_smoke_failed}" -gt "${MAX_STREAM_SMOKE_FAILURES}" ]]; then
  overall_failed=1
fi
if [[ "${canary_failed}" -gt "${MAX_CANARY_DRILL_FAILURES}" ]]; then
  overall_failed=1
fi
if [[ "${dual_run_failed}" -gt "${MAX_DUAL_RUN_FAILURES}" ]]; then
  overall_failed=1
fi

jq -s \
  --arg generated_at "${TIMESTAMP}" \
  --arg base_url "${BASE_URL}" \
  --arg stable_revision "${STABLE_REVISION}" \
  --arg canary_revision "${CANARY_REVISION}" \
  --argjson dry_run "$( [[ "${DRY_RUN}" == "1" ]] && echo true || echo false )" \
  --argjson apply_route_flip "$( [[ "${APPLY_ROUTE_FLIP}" == "1" ]] && echo true || echo false )" \
  --arg project "${PROJECT}" \
  --arg region "${REGION}" \
  --arg service "${SERVICE}" \
  --argjson run_dual_run "$( [[ "${RUN_DUAL_RUN}" == "1" ]] && echo true || echo false )" \
  --arg route_flip_failed "${route_flip_failed}" \
  --arg stream_smoke_failed "${stream_smoke_failed}" \
  --arg canary_failed "${canary_failed}" \
  --arg dual_run_failed "${dual_run_failed}" \
  --arg dual_run_p95_delta "${dual_run_p95_delta}" \
  --arg max_route_flip_failures "${MAX_ROUTE_FLIP_FAILURES}" \
  --arg max_stream_smoke_failures "${MAX_STREAM_SMOKE_FAILURES}" \
  --arg max_canary_drill_failures "${MAX_CANARY_DRILL_FAILURES}" \
  --arg max_dual_run_failures "${MAX_DUAL_RUN_FAILURES}" \
  --arg max_error_budget "${MAX_ERROR_BUDGET_CONSUMED_PERCENT}" \
  --arg observed_error_budget "${SLO_ERROR_BUDGET_CONSUMED_PERCENT}" \
  '
  def count_status(s): map(select(.status == s)) | length;
  {
    schema: "openagents.webparity.production_stream_cutover.v1",
    generated_at: $generated_at,
    base_url: $base_url,
    dry_run: $dry_run,
    apply_route_flip: $apply_route_flip,
    stable_revision: $stable_revision,
    canary_revision: $canary_revision,
    project: $project,
    region: $region,
    service: $service,
    run_dual_run: $run_dual_run,
    gates: {
      route_flip: {
        observed_failed: ($route_flip_failed | tonumber),
        max_failed: ($max_route_flip_failures | tonumber)
      },
      stream_smoke: {
        observed_failed: ($stream_smoke_failed | tonumber),
        max_failed: ($max_stream_smoke_failures | tonumber)
      },
      canary_drill: {
        observed_failed: ($canary_failed | tonumber),
        max_failed: ($max_canary_drill_failures | tonumber)
      },
      dual_run: {
        observed_failed: ($dual_run_failed | tonumber),
        max_failed: ($max_dual_run_failures | tonumber),
        observed_p95_delta_ms: ($dual_run_p95_delta | tonumber)
      },
      error_budget: {
        observed_consumed_percent: ($observed_error_budget | tonumber),
        max_consumed_percent: ($max_error_budget | tonumber)
      }
    },
    totals: {
      step_count: length,
      passed: count_status("pass"),
      failed: count_status("fail")
    },
    overall_status: (
      if
        ($route_flip_failed | tonumber) > ($max_route_flip_failures | tonumber) or
        ($stream_smoke_failed | tonumber) > ($max_stream_smoke_failures | tonumber) or
        ($canary_failed | tonumber) > ($max_canary_drill_failures | tonumber) or
        ($dual_run_failed | tonumber) > ($max_dual_run_failures | tonumber) or
        ($observed_error_budget | tonumber) > ($max_error_budget | tonumber) or
        count_status("fail") > 0
      then "failed"
      else "passed"
      end
    ),
    steps: .
  }
' "${RESULTS_JSONL}" >"${SUMMARY_JSON}"

{
  echo "# Production Stream Cutover"
  echo
  echo "- Generated at: ${TIMESTAMP}"
  echo "- Base URL: ${BASE_URL}"
  echo "- Dry run: ${DRY_RUN}"
  echo "- Apply route flip: ${APPLY_ROUTE_FLIP}"
  echo "- Stable revision: ${STABLE_REVISION}"
  echo "- Canary revision: ${CANARY_REVISION}"
  echo "- Overall status: $(jq -r '.overall_status' "${SUMMARY_JSON}")"
  echo "- Totals: $(jq -r '.totals.passed' "${SUMMARY_JSON}") pass / $(jq -r '.totals.failed' "${SUMMARY_JSON}") fail"
  echo "- Gate metrics:"
  echo "  - route_flip_failed: ${route_flip_failed} (max ${MAX_ROUTE_FLIP_FAILURES})"
  echo "  - stream_smoke_failed: ${stream_smoke_failed} (max ${MAX_STREAM_SMOKE_FAILURES})"
  echo "  - canary_failed: ${canary_failed} (max ${MAX_CANARY_DRILL_FAILURES})"
  echo "  - dual_run_failed: ${dual_run_failed} (max ${MAX_DUAL_RUN_FAILURES})"
  echo "  - error_budget_consumed: ${SLO_ERROR_BUDGET_CONSUMED_PERCENT}% (max ${MAX_ERROR_BUDGET_CONSUMED_PERCENT}%)"
  echo
  echo "| Step | Status | Detail | Artifact |"
  echo "| --- | --- | --- | --- |"
  jq -r '.steps[] | "| \(.step_id) | \(.status) | \(.detail) | \((.artifact // "") | if . == "" then "" else "`" + . + "`" end) |"' "${SUMMARY_JSON}"
} >"${SUMMARY_MD}"

echo "[stream-cutover] summary: ${SUMMARY_JSON}"
echo "[stream-cutover] report: ${SUMMARY_MD}"

if [[ "$(jq -r '.overall_status' "${SUMMARY_JSON}")" != "passed" ]]; then
  echo "error: production stream cutover gates failed" >&2
  exit 1
fi
