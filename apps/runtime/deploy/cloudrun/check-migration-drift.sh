#!/usr/bin/env bash
set -euo pipefail

SCRIPT_NAME="$(basename "$0")"

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
require_command jq

GCP_PROJECT="${GCP_PROJECT:-openagentsgemini}"
GCP_REGION="${GCP_REGION:-us-central1}"
RUNTIME_SERVICE="${RUNTIME_SERVICE:-runtime}"
MIGRATE_JOB="${MIGRATE_JOB:-runtime-migrate}"
EXPECTED_IMAGE="${EXPECTED_IMAGE:-}"

SERVICE_IMAGE="$(gcloud run services describe "${RUNTIME_SERVICE}" --project "${GCP_PROJECT}" --region "${GCP_REGION}" --format='value(spec.template.spec.containers[0].image)')"
JOB_IMAGE="$(gcloud run jobs describe "${MIGRATE_JOB}" --project "${GCP_PROJECT}" --region "${GCP_REGION}" --format='value(spec.template.spec.template.spec.containers[0].image)')"

if [[ -z "${SERVICE_IMAGE}" || -z "${JOB_IMAGE}" ]]; then
  echo "error: unable to resolve service/job images" >&2
  exit 1
fi

if [[ -n "${EXPECTED_IMAGE}" && "${SERVICE_IMAGE}" != "${EXPECTED_IMAGE}" ]]; then
  echo "error: runtime service image mismatch (expected=${EXPECTED_IMAGE}, actual=${SERVICE_IMAGE})" >&2
  exit 1
fi

if [[ "${SERVICE_IMAGE}" != "${JOB_IMAGE}" ]]; then
  echo "error: migration job image drift detected (service=${SERVICE_IMAGE}, job=${JOB_IMAGE})" >&2
  exit 1
fi

LATEST_EXEC_JSON="$(gcloud run jobs executions list --job "${MIGRATE_JOB}" --project "${GCP_PROJECT}" --region "${GCP_REGION}" --limit=1 --format=json)"
EXEC_COUNT="$(echo "${LATEST_EXEC_JSON}" | jq 'length')"
if [[ "${EXEC_COUNT}" == "0" ]]; then
  echo "error: no migration job executions found for ${MIGRATE_JOB}" >&2
  exit 1
fi

LATEST_EXEC_IMAGE="$(echo "${LATEST_EXEC_JSON}" | jq -r '.[0].spec.template.spec.containers[0].image // .[0].template.containers[0].image // empty')"
LATEST_EXEC_NAME="$(echo "${LATEST_EXEC_JSON}" | jq -r '.[0].metadata.name // .[0].name // "(unknown)"')"
LATEST_EXEC_SUCCESS="$(echo "${LATEST_EXEC_JSON}" | jq -r '([.[0].conditions[]? | select((.type=="Completed" or .type=="Succeeded") and (.state=="CONDITION_SUCCEEDED" or .status=="True"))] | length)')"

if [[ "${LATEST_EXEC_SUCCESS}" == "0" ]]; then
  echo "error: latest migration execution is not marked successful (${LATEST_EXEC_NAME})" >&2
  exit 1
fi

if [[ -n "${LATEST_EXEC_IMAGE}" && "${LATEST_EXEC_IMAGE}" != "${SERVICE_IMAGE}" ]]; then
  echo "error: latest migration execution image mismatch (execution=${LATEST_EXEC_IMAGE}, service=${SERVICE_IMAGE}, execution_name=${LATEST_EXEC_NAME})" >&2
  exit 1
fi

log "migration drift check passed"
log "service_image=${SERVICE_IMAGE} job_image=${JOB_IMAGE} latest_execution=${LATEST_EXEC_NAME}"
