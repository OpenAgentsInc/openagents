#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/docs/reports/rust-cutover-validation/${TIMESTAMP}}"
LOG_DIR="${OUTPUT_DIR}/logs"
RESULTS_TSV="${OUTPUT_DIR}/results.tsv"
SUMMARY_MD="${OUTPUT_DIR}/SUMMARY.md"

STAGING_CONTROL_BASE_URL="${STAGING_CONTROL_BASE_URL:-https://staging.openagents.com}"
PROD_CONTROL_BASE_URL="${PROD_CONTROL_BASE_URL:-https://openagents.com}"
STAGING_RUNTIME_BASE_URL="${STAGING_RUNTIME_BASE_URL:-}"
PROD_RUNTIME_BASE_URL="${PROD_RUNTIME_BASE_URL:-}"

STAGING_CONTROL_ACCESS_TOKEN="${STAGING_CONTROL_ACCESS_TOKEN:-}"
PROD_CONTROL_ACCESS_TOKEN="${PROD_CONTROL_ACCESS_TOKEN:-}"

GCP_PROJECT="${GCP_PROJECT:-openagentsgemini}"
GCP_REGION="${GCP_REGION:-us-central1}"
RUNTIME_SERVICE="${RUNTIME_SERVICE:-runtime}"
MIGRATE_JOB="${MIGRATE_JOB:-runtime-migrate}"
CONTROL_SERVICE="${CONTROL_SERVICE:-openagents-control-service}"

RUN_RUNTIME_DRIFT_CHECK="${RUN_RUNTIME_DRIFT_CHECK:-1}"
RUN_KHALA_CONTRACT_TESTS="${RUN_KHALA_CONTRACT_TESTS:-1}"
RUN_SPACETIME_SYNC_CONTRACT_TESTS="${RUN_SPACETIME_SYNC_CONTRACT_TESTS:-${RUN_KHALA_CONTRACT_TESTS}}"
RUN_CROSS_SURFACE="${RUN_CROSS_SURFACE:-0}"
RUN_LOG_PROBES="${RUN_LOG_PROBES:-1}"
RUN_HTMX_PERF_CHECKS="${RUN_HTMX_PERF_CHECKS:-1}"
RUN_HTMX_BROWSER_SMOKE="${RUN_HTMX_BROWSER_SMOKE:-1}"
RUN_HTMX_ROUTE_GROUP_CANARY="${RUN_HTMX_ROUTE_GROUP_CANARY:-1}"
FAIL_ON_REQUIRED_FAILURE="${FAIL_ON_REQUIRED_FAILURE:-1}"

mkdir -p "${LOG_DIR}"
printf 'check_id\trequired\tstatus\tdescription\tlog_path\tcommand\n' >"${RESULTS_TSV}"

run_check() {
  local check_id="$1"
  local required="$2"
  local description="$3"
  shift 3

  local log_path="${LOG_DIR}/${check_id}.log"
  local cmd_display
  cmd_display="$(printf '%q ' "$@")"

  local status="passed"
  if ! "$@" >"${log_path}" 2>&1; then
    status="failed"
  fi

  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "${check_id}" \
    "${required}" \
    "${status}" \
    "${description}" \
    "${log_path}" \
    "${cmd_display}" >>"${RESULTS_TSV}"

  echo "[${status}] ${check_id} (${required})"
}

skip_check() {
  local check_id="$1"
  local required="$2"
  local description="$3"
  local reason="$4"
  local log_path="${LOG_DIR}/${check_id}.log"
  printf '%s\n' "${reason}" >"${log_path}"
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
    "${check_id}" \
    "${required}" \
    "skipped" \
    "${description}" \
    "${log_path}" \
    "${reason}" >>"${RESULTS_TSV}"
  echo "[skipped] ${check_id} (${required})"
}

# A) Control service checks (staging + production).
run_check \
  "control-staging-health" \
  "required" \
  "staging control health/ready endpoints" \
  env OPENAGENTS_BASE_URL="${STAGING_CONTROL_BASE_URL}" \
  "${ROOT_DIR}/apps/openagents.com/service/deploy/smoke-health.sh"

run_check \
  "control-staging-smoke" \
  "required" \
  "staging control smoke (static host + auth/session/sync token checks when token provided)" \
  env OPENAGENTS_BASE_URL="${STAGING_CONTROL_BASE_URL}" OPENAGENTS_CONTROL_ACCESS_TOKEN="${STAGING_CONTROL_ACCESS_TOKEN}" \
  "${ROOT_DIR}/apps/openagents.com/service/deploy/smoke-control.sh"

run_check \
  "control-prod-health" \
  "required" \
  "production control health/ready endpoints" \
  env OPENAGENTS_BASE_URL="${PROD_CONTROL_BASE_URL}" \
  "${ROOT_DIR}/apps/openagents.com/service/deploy/smoke-health.sh"

run_check \
  "control-prod-smoke" \
  "required" \
  "production control smoke (static host + auth/session/sync token checks when token provided)" \
  env OPENAGENTS_BASE_URL="${PROD_CONTROL_BASE_URL}" OPENAGENTS_CONTROL_ACCESS_TOKEN="${PROD_CONTROL_ACCESS_TOKEN}" \
  "${ROOT_DIR}/apps/openagents.com/service/deploy/smoke-control.sh"

# A2) HTMX performance lane (staging + production).
if [[ "${RUN_HTMX_PERF_CHECKS}" == "1" ]]; then
  if [[ -n "${STAGING_CONTROL_ACCESS_TOKEN}" ]]; then
    run_check \
      "control-staging-htmx-perf" \
      "required" \
      "staging HTMX perf budget check (login/feed/settings/chat)" \
      env BASE_URL="${STAGING_CONTROL_BASE_URL}" OA_ACCESS_TOKEN="${STAGING_CONTROL_ACCESS_TOKEN}" REQUIRE_AUTH_FLOWS=1 \
      "${ROOT_DIR}/apps/openagents.com/service/scripts/htmx_perf_check.sh"
  else
    skip_check \
      "control-staging-htmx-perf" \
      "required" \
      "staging HTMX perf budget check (login/feed/settings/chat)" \
      "STAGING_CONTROL_ACCESS_TOKEN unset"
  fi

  if [[ -n "${PROD_CONTROL_ACCESS_TOKEN}" ]]; then
    run_check \
      "control-prod-htmx-perf" \
      "required" \
      "production HTMX perf budget check (login/feed/settings/chat)" \
      env BASE_URL="${PROD_CONTROL_BASE_URL}" OA_ACCESS_TOKEN="${PROD_CONTROL_ACCESS_TOKEN}" REQUIRE_AUTH_FLOWS=1 \
      "${ROOT_DIR}/apps/openagents.com/service/scripts/htmx_perf_check.sh"
  else
    skip_check \
      "control-prod-htmx-perf" \
      "required" \
      "production HTMX perf budget check (login/feed/settings/chat)" \
      "PROD_CONTROL_ACCESS_TOKEN unset"
  fi
else
  skip_check \
    "control-staging-htmx-perf" \
    "required" \
    "staging HTMX perf budget check (login/feed/settings/chat)" \
    "RUN_HTMX_PERF_CHECKS=0"
  skip_check \
    "control-prod-htmx-perf" \
    "required" \
    "production HTMX perf budget check (login/feed/settings/chat)" \
    "RUN_HTMX_PERF_CHECKS=0"
fi

# A3) HTMX browser smoke lane (staging + production).
if [[ "${RUN_HTMX_BROWSER_SMOKE}" == "1" ]]; then
  if command -v node >/dev/null 2>&1; then
    if [[ -n "${STAGING_CONTROL_ACCESS_TOKEN}" ]]; then
      run_check \
        "control-staging-htmx-browser-smoke" \
        "required" \
        "staging browser-level HTMX smoke (chat/feed/settings URL + fragment/history behaviors)" \
        env BASE_URL="${STAGING_CONTROL_BASE_URL}" OA_BROWSER_SMOKE_ACCESS_TOKEN="${STAGING_CONTROL_ACCESS_TOKEN}" OA_BROWSER_SMOKE_REQUIRE_LOGIN_FLOW=0 OA_BROWSER_SMOKE_ARTIFACT_DIR="${OUTPUT_DIR}/artifacts/control-staging-htmx-browser-smoke" \
        "${ROOT_DIR}/apps/openagents.com/service/scripts/htmx_browser_smoke.sh"
    else
      skip_check \
        "control-staging-htmx-browser-smoke" \
        "required" \
        "staging browser-level HTMX smoke (chat/feed/settings URL + fragment/history behaviors)" \
        "STAGING_CONTROL_ACCESS_TOKEN unset"
    fi

    if [[ -n "${PROD_CONTROL_ACCESS_TOKEN}" ]]; then
      run_check \
        "control-prod-htmx-browser-smoke" \
        "required" \
        "production browser-level HTMX smoke (chat/feed/settings URL + fragment/history behaviors)" \
        env BASE_URL="${PROD_CONTROL_BASE_URL}" OA_BROWSER_SMOKE_ACCESS_TOKEN="${PROD_CONTROL_ACCESS_TOKEN}" OA_BROWSER_SMOKE_REQUIRE_LOGIN_FLOW=0 OA_BROWSER_SMOKE_ARTIFACT_DIR="${OUTPUT_DIR}/artifacts/control-prod-htmx-browser-smoke" \
        "${ROOT_DIR}/apps/openagents.com/service/scripts/htmx_browser_smoke.sh"
    else
      skip_check \
        "control-prod-htmx-browser-smoke" \
        "required" \
        "production browser-level HTMX smoke (chat/feed/settings URL + fragment/history behaviors)" \
        "PROD_CONTROL_ACCESS_TOKEN unset"
    fi
  else
    skip_check \
      "control-staging-htmx-browser-smoke" \
      "required" \
      "staging browser-level HTMX smoke (chat/feed/settings URL + fragment/history behaviors)" \
      "node not installed"
    skip_check \
      "control-prod-htmx-browser-smoke" \
      "required" \
      "production browser-level HTMX smoke (chat/feed/settings URL + fragment/history behaviors)" \
      "node not installed"
  fi
else
  skip_check \
    "control-staging-htmx-browser-smoke" \
    "required" \
    "staging browser-level HTMX smoke (chat/feed/settings URL + fragment/history behaviors)" \
    "RUN_HTMX_BROWSER_SMOKE=0"
  skip_check \
    "control-prod-htmx-browser-smoke" \
    "required" \
    "production browser-level HTMX smoke (chat/feed/settings URL + fragment/history behaviors)" \
    "RUN_HTMX_BROWSER_SMOKE=0"
fi

# A4) HTMX route-group staged canary lane (staging only).
if [[ "${RUN_HTMX_ROUTE_GROUP_CANARY}" == "1" ]]; then
  if [[ -n "${STAGING_CONTROL_ACCESS_TOKEN}" ]]; then
    run_check \
      "control-staging-htmx-route-group-canary" \
      "required" \
      "staging route-group HTMX staged rollout/rollback canary" \
      env BASE_URL="${STAGING_CONTROL_BASE_URL}" CONTROL_ACCESS_TOKEN="${STAGING_CONTROL_ACCESS_TOKEN}" \
      "${ROOT_DIR}/apps/openagents.com/service/scripts/htmx-route-group-canary.sh"
  else
    skip_check \
      "control-staging-htmx-route-group-canary" \
      "required" \
      "staging route-group HTMX staged rollout/rollback canary" \
      "STAGING_CONTROL_ACCESS_TOKEN unset"
  fi
else
  skip_check \
    "control-staging-htmx-route-group-canary" \
    "required" \
    "staging route-group HTMX staged rollout/rollback canary" \
    "RUN_HTMX_ROUTE_GROUP_CANARY=0"
fi

# B) Runtime service checks.
if [[ -n "${STAGING_RUNTIME_BASE_URL}" ]]; then
  run_check \
    "runtime-staging-smoke" \
    "required" \
    "staging runtime health and authority API smoke" \
    env SMOKE_BASE_URL="${STAGING_RUNTIME_BASE_URL}" \
    cargo run --manifest-path "${ROOT_DIR}/apps/runtime/Cargo.toml" --bin runtime-smoke
else
  skip_check \
    "runtime-staging-smoke" \
    "required" \
    "staging runtime health and authority API smoke" \
    "STAGING_RUNTIME_BASE_URL unset"
fi

if [[ -n "${PROD_RUNTIME_BASE_URL}" ]]; then
  run_check \
    "runtime-prod-smoke" \
    "required" \
    "production runtime health and authority API smoke" \
    env SMOKE_BASE_URL="${PROD_RUNTIME_BASE_URL}" \
    cargo run --manifest-path "${ROOT_DIR}/apps/runtime/Cargo.toml" --bin runtime-smoke
else
  skip_check \
    "runtime-prod-smoke" \
    "required" \
    "production runtime health and authority API smoke" \
    "PROD_RUNTIME_BASE_URL unset"
fi

if [[ "${RUN_RUNTIME_DRIFT_CHECK}" == "1" ]]; then
  if command -v gcloud >/dev/null 2>&1; then
    run_check \
      "runtime-migration-drift" \
      "required" \
      "runtime service/migrate-job image + latest execution drift check" \
      env GCP_PROJECT="${GCP_PROJECT}" GCP_REGION="${GCP_REGION}" RUNTIME_SERVICE="${RUNTIME_SERVICE}" MIGRATE_JOB="${MIGRATE_JOB}" \
      "${ROOT_DIR}/apps/runtime/deploy/cloudrun/check-migration-drift.sh"
  else
    skip_check \
      "runtime-migration-drift" \
      "required" \
      "runtime service/migrate-job image + latest execution drift check" \
      "gcloud not installed"
  fi
else
  skip_check \
    "runtime-migration-drift" \
    "required" \
    "runtime service/migrate-job image + latest execution drift check" \
    "RUN_RUNTIME_DRIFT_CHECK=0"
fi

# C) Spacetime sync contract lane.
if [[ "${RUN_SPACETIME_SYNC_CONTRACT_TESTS}" == "1" ]]; then
  run_check \
    "spacetime-sync-contract-tests" \
    "required" \
    "runtime spacetime sync contract tests" \
    cargo test --manifest-path "${ROOT_DIR}/apps/runtime/Cargo.toml" server::tests::spacetime_sync_metrics_expose_stream_delivery_totals -- --nocapture

  run_check \
    "spacetime-retired-route-guards" \
    "required" \
    "retired khala internal routes remain removed" \
    cargo test --manifest-path "${ROOT_DIR}/apps/runtime/Cargo.toml" server::tests::retired_khala_routes_return_not_found -- --nocapture
else
  skip_check \
    "spacetime-sync-contract-tests" \
    "required" \
    "runtime spacetime sync contract tests" \
    "RUN_SPACETIME_SYNC_CONTRACT_TESTS=0"

  skip_check \
    "spacetime-retired-route-guards" \
    "required" \
    "retired khala internal routes remain removed" \
    "RUN_SPACETIME_SYNC_CONTRACT_TESTS=0"
fi

# D) Surface parity lane.
if [[ "${RUN_CROSS_SURFACE}" == "1" ]]; then
  run_check \
    "cross-surface-contract-harness" \
    "optional" \
    "desktop/runtime-api cross-surface contract harness" \
    "${ROOT_DIR}/scripts/run-cross-surface-contract-harness.sh"
else
  skip_check \
    "cross-surface-contract-harness" \
    "optional" \
    "desktop/runtime-api cross-surface contract harness" \
    "RUN_CROSS_SURFACE=0"
fi

# E) Observability + rollback readiness probes.
if [[ "${RUN_LOG_PROBES}" == "1" ]]; then
  if command -v gcloud >/dev/null 2>&1; then
    run_check \
      "control-error-log-probe" \
      "required" \
      "control service error log probe (last 10m)" \
      gcloud logging read "resource.type=cloud_run_revision AND resource.labels.service_name=${CONTROL_SERVICE} AND severity>=ERROR" \
      --project "${GCP_PROJECT}" \
      --freshness=10m \
      --limit=50 \
      --format=json

    run_check \
      "runtime-error-log-probe" \
      "required" \
      "runtime service error log probe (last 10m)" \
      gcloud logging read "resource.type=cloud_run_revision AND resource.labels.service_name=${RUNTIME_SERVICE} AND severity>=ERROR" \
      --project "${GCP_PROJECT}" \
      --freshness=10m \
      --limit=50 \
      --format=json

    run_check \
      "control-canary-status" \
      "required" \
      "control service canary status probe" \
      env PROJECT="${GCP_PROJECT}" REGION="${GCP_REGION}" SERVICE="${CONTROL_SERVICE}" \
      "${ROOT_DIR}/apps/openagents.com/service/deploy/canary-rollout.sh" status
  else
    skip_check \
      "control-error-log-probe" \
      "required" \
      "control service error log probe (last 10m)" \
      "gcloud not installed"
    skip_check \
      "runtime-error-log-probe" \
      "required" \
      "runtime service error log probe (last 10m)" \
      "gcloud not installed"
    skip_check \
      "control-canary-status" \
      "required" \
      "control service canary status probe" \
      "gcloud not installed"
  fi
else
  skip_check \
    "control-error-log-probe" \
    "required" \
    "control service error log probe (last 10m)" \
    "RUN_LOG_PROBES=0"
  skip_check \
    "runtime-error-log-probe" \
    "required" \
    "runtime service error log probe (last 10m)" \
    "RUN_LOG_PROBES=0"
  skip_check \
    "control-canary-status" \
    "required" \
    "control service canary status probe" \
    "RUN_LOG_PROBES=0"
fi

required_failures=0
optional_failures=0
skipped_required=0
skipped_optional=0
passed_checks=0
total_checks=0

while IFS=$'\t' read -r check_id required status description log_path command; do
  if [[ "${check_id}" == "check_id" ]]; then
    continue
  fi
  total_checks=$((total_checks + 1))
  case "${status}" in
    passed)
      passed_checks=$((passed_checks + 1))
      ;;
    failed)
      if [[ "${required}" == "required" ]]; then
        required_failures=$((required_failures + 1))
      else
        optional_failures=$((optional_failures + 1))
      fi
      ;;
    skipped)
      if [[ "${required}" == "required" ]]; then
        skipped_required=$((skipped_required + 1))
      else
        skipped_optional=$((skipped_optional + 1))
      fi
      ;;
  esac
done <"${RESULTS_TSV}"

overall="passed"
if [[ "${required_failures}" -gt 0 || "${skipped_required}" -gt 0 ]]; then
  overall="failed"
fi

{
  echo "# Rust Staging/Prod Validation Matrix"
  echo
  echo "- Timestamp (UTC): ${TIMESTAMP}"
  echo "- Overall: ${overall}"
  echo "- Total checks: ${total_checks}"
  echo "- Passed: ${passed_checks}"
  echo "- Required failures: ${required_failures}"
  echo "- Optional failures: ${optional_failures}"
  echo "- Required skipped: ${skipped_required}"
  echo "- Optional skipped: ${skipped_optional}"
  echo
  echo "## Environment"
  echo
  echo "- STAGING_CONTROL_BASE_URL=${STAGING_CONTROL_BASE_URL}"
  echo "- PROD_CONTROL_BASE_URL=${PROD_CONTROL_BASE_URL}"
  echo "- STAGING_RUNTIME_BASE_URL=${STAGING_RUNTIME_BASE_URL:-<unset>}"
  echo "- PROD_RUNTIME_BASE_URL=${PROD_RUNTIME_BASE_URL:-<unset>}"
  echo "- GCP_PROJECT=${GCP_PROJECT}"
  echo "- GCP_REGION=${GCP_REGION}"
  echo "- CONTROL_SERVICE=${CONTROL_SERVICE}"
  echo "- RUNTIME_SERVICE=${RUNTIME_SERVICE}"
  echo "- MIGRATE_JOB=${MIGRATE_JOB}"
  echo
  echo "## Check Results"
  echo
  echo "| Check | Required | Status | Description | Log |"
  echo "| --- | --- | --- | --- | --- |"

  while IFS=$'\t' read -r check_id required status description log_path command; do
    if [[ "${check_id}" == "check_id" ]]; then
      continue
    fi
    rel_log="${log_path#${ROOT_DIR}/}"
    echo "| ${check_id} | ${required} | ${status} | ${description} | ${rel_log} |"
  done <"${RESULTS_TSV}"

  echo
  echo "## Commands"
  echo
  while IFS=$'\t' read -r check_id required status description log_path command; do
    if [[ "${check_id}" == "check_id" ]]; then
      continue
    fi
    echo "- ${check_id}: ${command}"
  done <"${RESULTS_TSV}"
} >"${SUMMARY_MD}"

echo "[matrix] results: ${RESULTS_TSV}"
echo "[matrix] summary: ${SUMMARY_MD}"

if [[ "${FAIL_ON_REQUIRED_FAILURE}" == "1" && "${overall}" != "passed" ]]; then
  exit 1
fi
