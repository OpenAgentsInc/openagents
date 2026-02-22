#!/usr/bin/env bash
set -euo pipefail

PROJECT="${PROJECT:-}"
REGION="${REGION:-us-central1}"
DRY_RUN="${DRY_RUN:-1}"

RUN_JOBS_CSV="${RUN_JOBS_CSV:-openagents-queue,openagents-scheduler}"
SCHEDULER_JOBS_CSV="${SCHEDULER_JOBS_CSV:-openagents-scheduler}"

if [[ -z "${PROJECT}" ]]; then
  echo "error: PROJECT is required (GCP project id)." >&2
  exit 1
fi

if ! command -v gcloud >/dev/null 2>&1; then
  echo "error: gcloud is required." >&2
  exit 1
fi

run_cmd() {
  echo "+ $*"
  if [[ "${DRY_RUN}" == "1" ]]; then
    return 0
  fi
  "$@"
}

describe_run_job() {
  local name="$1"
  gcloud run jobs describe "$name" --project "${PROJECT}" --region "${REGION}" >/dev/null 2>&1
}

describe_scheduler_job() {
  local name="$1"
  gcloud scheduler jobs describe "$name" --project "${PROJECT}" --location "${REGION}" >/dev/null 2>&1
}

IFS=',' read -r -a RUN_JOBS <<<"${RUN_JOBS_CSV}"
for name in "${RUN_JOBS[@]}"; do
  name="$(echo "$name" | xargs)"
  [[ -z "${name}" ]] && continue
  if describe_run_job "${name}"; then
    run_cmd gcloud run jobs delete "${name}" --project "${PROJECT}" --region "${REGION}" --quiet
  else
    echo "skip: Cloud Run job not found: ${name}"
  fi
done

IFS=',' read -r -a SCHEDULER_JOBS <<<"${SCHEDULER_JOBS_CSV}"
for name in "${SCHEDULER_JOBS[@]}"; do
  name="$(echo "$name" | xargs)"
  [[ -z "${name}" ]] && continue
  if describe_scheduler_job "${name}"; then
    run_cmd gcloud scheduler jobs pause "${name}" --project "${PROJECT}" --location "${REGION}"
  else
    echo "skip: Cloud Scheduler job not found: ${name}"
  fi
done

echo "disable-legacy-laravel-async-jobs: completed (DRY_RUN=${DRY_RUN})"
