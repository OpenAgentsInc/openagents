#!/usr/bin/env bash
set -euo pipefail

SCRIPT_NAME="$(basename "$0")"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUN_MIGRATE_SCRIPT="${SCRIPT_DIR}/run-migrate-job.sh"
DRIFT_CHECK_SCRIPT="${SCRIPT_DIR}/check-migration-drift.sh"

log() {
  echo "[${SCRIPT_NAME}] $*"
}

require_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "error: required command not found: $command_name" >&2
    exit 1
  fi
}

require_command gcloud

GCP_PROJECT="${GCP_PROJECT:-openagentsgemini}"
GCP_REGION="${GCP_REGION:-us-central1}"
RUNTIME_SERVICE="${RUNTIME_SERVICE:-runtime}"
MIGRATE_JOB="${MIGRATE_JOB:-runtime-migrate}"
IMAGE="${IMAGE:-}"
DEPLOY_NO_TRAFFIC="${DEPLOY_NO_TRAFFIC:-0}"
RUNTIME_DEPLOY_EXTRA_ARGS="${RUNTIME_DEPLOY_EXTRA_ARGS:-}"
VERIFY_DB_ROLE_ISOLATION="${VERIFY_DB_ROLE_ISOLATION:-1}"
VERIFY_IMAGE_ALIGNMENT="${VERIFY_IMAGE_ALIGNMENT:-1}"
DRY_RUN="${DRY_RUN:-0}"
RECEIPT_PATH="${RECEIPT_PATH:-}"

if [[ -z "${IMAGE}" ]]; then
  echo "error: IMAGE is required" >&2
  exit 1
fi

DEPLOY_CMD=(
  gcloud run deploy "${RUNTIME_SERVICE}"
  --project "${GCP_PROJECT}"
  --region "${GCP_REGION}"
  --image "${IMAGE}"
  --quiet
)

if [[ "${DEPLOY_NO_TRAFFIC}" == "1" ]]; then
  DEPLOY_CMD+=(--no-traffic)
fi

if [[ -n "${RUNTIME_DEPLOY_EXTRA_ARGS}" ]]; then
  # shellcheck disable=SC2206
  EXTRA_ARGS=( ${RUNTIME_DEPLOY_EXTRA_ARGS} )
  DEPLOY_CMD+=("${EXTRA_ARGS[@]}")
fi

if [[ "${DRY_RUN}" == "1" ]]; then
  log "DRY_RUN=1; planned deploy command:"
  printf '%q ' "${DEPLOY_CMD[@]}"
  echo
  log "DRY_RUN=1; planned migration command:"
  printf '%q ' \
    GCP_PROJECT="${GCP_PROJECT}" \
    GCP_REGION="${GCP_REGION}" \
    RUNTIME_SERVICE="${RUNTIME_SERVICE}" \
    MIGRATE_JOB="${MIGRATE_JOB}" \
    IMAGE="${IMAGE}" \
    VERIFY_DB_ROLE_ISOLATION="${VERIFY_DB_ROLE_ISOLATION}" \
    "${RUN_MIGRATE_SCRIPT}"
  echo
  log "DRY_RUN=1; planned drift-check command:"
  printf '%q ' \
    GCP_PROJECT="${GCP_PROJECT}" \
    GCP_REGION="${GCP_REGION}" \
    RUNTIME_SERVICE="${RUNTIME_SERVICE}" \
    MIGRATE_JOB="${MIGRATE_JOB}" \
    EXPECTED_IMAGE="${IMAGE}" \
    "${DRIFT_CHECK_SCRIPT}"
  echo
  exit 0
fi

log "Deploying runtime service ${RUNTIME_SERVICE} with image ${IMAGE}"
"${DEPLOY_CMD[@]}"

DEPLOYED_IMAGE="$(gcloud run services describe "${RUNTIME_SERVICE}" --project "${GCP_PROJECT}" --region "${GCP_REGION}" --format='value(spec.template.spec.containers[0].image)')"
if [[ "${DEPLOYED_IMAGE}" != "${IMAGE}" ]]; then
  echo "error: runtime service image mismatch after deploy (expected=${IMAGE}, actual=${DEPLOYED_IMAGE})" >&2
  exit 1
fi

log "Running mandatory migration step"
GCP_PROJECT="${GCP_PROJECT}" \
GCP_REGION="${GCP_REGION}" \
RUNTIME_SERVICE="${RUNTIME_SERVICE}" \
MIGRATE_JOB="${MIGRATE_JOB}" \
IMAGE="${IMAGE}" \
VERIFY_DB_ROLE_ISOLATION="${VERIFY_DB_ROLE_ISOLATION}" \
DB_URL="${DB_URL:-${DATABASE_URL:-}}" \
"${RUN_MIGRATE_SCRIPT}"

if [[ "${VERIFY_IMAGE_ALIGNMENT}" == "1" ]]; then
  GCP_PROJECT="${GCP_PROJECT}" \
  GCP_REGION="${GCP_REGION}" \
  RUNTIME_SERVICE="${RUNTIME_SERVICE}" \
  MIGRATE_JOB="${MIGRATE_JOB}" \
  EXPECTED_IMAGE="${DEPLOYED_IMAGE}" \
  "${DRIFT_CHECK_SCRIPT}"
fi

if [[ -n "${RECEIPT_PATH}" ]]; then
  mkdir -p "$(dirname "${RECEIPT_PATH}")"
  cat >"${RECEIPT_PATH}" <<JSON
{
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "project": "${GCP_PROJECT}",
  "region": "${GCP_REGION}",
  "runtime_service": "${RUNTIME_SERVICE}",
  "migrate_job": "${MIGRATE_JOB}",
  "image": "${IMAGE}",
  "deployed_image": "${DEPLOYED_IMAGE}",
  "verify_db_role_isolation": ${VERIFY_DB_ROLE_ISOLATION},
  "verify_image_alignment": ${VERIFY_IMAGE_ALIGNMENT}
}
JSON
  log "wrote receipt: ${RECEIPT_PATH}"
fi

log "Runtime deploy + migration validation complete"
