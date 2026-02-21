#!/usr/bin/env bash
set -euo pipefail

SCRIPT_NAME="$(basename "$0")"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROLE_ISOLATION_VERIFY_SCRIPT="${SCRIPT_DIR}/verify-db-role-isolation.sh"

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
DRY_RUN="${DRY_RUN:-0}"
VERIFY_DB_ROLE_ISOLATION="${VERIFY_DB_ROLE_ISOLATION:-1}"

service_image() {
  gcloud run services describe "${RUNTIME_SERVICE}" \
    --project "${GCP_PROJECT}" \
    --region "${GCP_REGION}" \
    --format='value(spec.template.spec.containers[0].image)'
}

job_image() {
  gcloud run jobs describe "${MIGRATE_JOB}" \
    --project "${GCP_PROJECT}" \
    --region "${GCP_REGION}" \
    --format='value(spec.template.spec.template.spec.containers[0].image)'
}

if [[ -z "${IMAGE}" ]]; then
  IMAGE="$(service_image)"
fi

if [[ -z "${IMAGE}" ]]; then
  echo "error: unable to resolve runtime image (set IMAGE or deploy ${RUNTIME_SERVICE} first)" >&2
  exit 1
fi

if [[ "${DRY_RUN}" == "1" ]]; then
  log "DRY_RUN=1; planned commands:"
  printf '%q ' \
    gcloud run jobs update "${MIGRATE_JOB}" \
      --project "${GCP_PROJECT}" \
      --region "${GCP_REGION}" \
      --image "${IMAGE}" \
      --command runtime-migrate \
      --quiet
  echo
  printf '%q ' \
    gcloud run jobs execute "${MIGRATE_JOB}" \
      --project "${GCP_PROJECT}" \
      --region "${GCP_REGION}" \
      --wait \
      --format='value(metadata.name)'
  echo
  exit 0
fi

CURRENT_JOB_IMAGE="$(job_image)"
log "Runtime service image: $(service_image)"
log "Current migrate job image: ${CURRENT_JOB_IMAGE}"
log "Target migrate job image: ${IMAGE}"

gcloud run jobs update "${MIGRATE_JOB}" \
  --project "${GCP_PROJECT}" \
  --region "${GCP_REGION}" \
  --image "${IMAGE}" \
  --command runtime-migrate \
  --quiet

UPDATED_JOB_IMAGE="$(job_image)"
if [[ "${UPDATED_JOB_IMAGE}" != "${IMAGE}" ]]; then
  echo "error: migrate job image mismatch after update (expected ${IMAGE}, got ${UPDATED_JOB_IMAGE})" >&2
  exit 1
fi

set +e
EXECUTION_OUTPUT="$(
  gcloud run jobs execute "${MIGRATE_JOB}" \
    --project "${GCP_PROJECT}" \
    --region "${GCP_REGION}" \
    --wait \
    --format='value(metadata.name)' 2>&1
)"
EXECUTION_STATUS=$?
set -e

if [[ ${EXECUTION_STATUS} -ne 0 ]]; then
  echo "${EXECUTION_OUTPUT}" >&2
  FAILED_EXECUTION_NAME="$(echo "${EXECUTION_OUTPUT}" | grep -Eo "${MIGRATE_JOB}-[a-z0-9]+" | tail -n 1 || true)"
  if [[ -n "${FAILED_EXECUTION_NAME}" ]]; then
    echo "Inspect failure logs:" >&2
    echo "gcloud logging read 'resource.type=\"cloud_run_job\" AND resource.labels.job_name=\"${MIGRATE_JOB}\" AND labels.\"run.googleapis.com/execution_name\"=\"${FAILED_EXECUTION_NAME}\"' --project \"${GCP_PROJECT}\" --limit=200 --format='table(timestamp,severity,textPayload)'" >&2
  fi
  exit ${EXECUTION_STATUS}
fi

EXECUTION_NAME="$(echo "${EXECUTION_OUTPUT}" | tr -d '\r' | tail -n 1)"
if [[ -z "${EXECUTION_NAME}" ]]; then
  echo "error: migration job execution name was empty" >&2
  exit 1
fi

log "Migration succeeded via execution: ${EXECUTION_NAME}"
log "Inspect execution logs:"
echo "gcloud logging read 'resource.type=\"cloud_run_job\" AND resource.labels.job_name=\"${MIGRATE_JOB}\" AND labels.\"run.googleapis.com/execution_name\"=\"${EXECUTION_NAME}\"' --project \"${GCP_PROJECT}\" --limit=200 --format='table(timestamp,severity,textPayload)'"

if [[ "${VERIFY_DB_ROLE_ISOLATION}" == "1" ]]; then
  if [[ -x "${ROLE_ISOLATION_VERIFY_SCRIPT}" ]]; then
    if [[ -n "${DB_URL:-}" || -n "${DATABASE_URL:-}" ]]; then
      DB_URL="${DB_URL:-${DATABASE_URL:-}}" "${ROLE_ISOLATION_VERIFY_SCRIPT}"
    else
      log "VERIFY_DB_ROLE_ISOLATION=1 but DB_URL/DATABASE_URL is unset; skipping role isolation verification."
    fi
  else
    log "VERIFY_DB_ROLE_ISOLATION=1 but verifier script missing: ${ROLE_ISOLATION_VERIFY_SCRIPT}"
  fi
fi
