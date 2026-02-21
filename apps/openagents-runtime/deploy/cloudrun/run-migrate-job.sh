#!/usr/bin/env bash
set -euo pipefail

SCRIPT_NAME="$(basename "$0")"

log() {
  echo "[$SCRIPT_NAME] $*"
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
RUNTIME_SERVICE="${RUNTIME_SERVICE:-openagents-runtime}"
MIGRATE_JOB="${MIGRATE_JOB:-openagents-runtime-migrate}"
IMAGE="${IMAGE:-}"
DRY_RUN="${DRY_RUN:-0}"
DEFAULT_MIGRATION_EVAL="OpenAgentsRuntime.Release.migrate_and_verify!()"
FALLBACK_MIGRATION_EVAL="OpenAgentsRuntime.Release.migrate()"
MIGRATION_EVAL="${MIGRATION_EVAL:-$DEFAULT_MIGRATION_EVAL}"

service_image() {
  gcloud run services describe "$RUNTIME_SERVICE" \
    --project "$GCP_PROJECT" \
    --region "$GCP_REGION" \
    --format='value(spec.template.spec.containers[0].image)'
}

job_image() {
  gcloud run jobs describe "$MIGRATE_JOB" \
    --project "$GCP_PROJECT" \
    --region "$GCP_REGION" \
    --format='value(spec.template.spec.template.spec.containers[0].image)'
}

if [[ -z "$IMAGE" ]]; then
  IMAGE="$(service_image)"
fi

if [[ -z "$IMAGE" ]]; then
  echo "error: unable to resolve runtime image (set IMAGE or deploy $RUNTIME_SERVICE first)" >&2
  exit 1
fi

CURRENT_JOB_IMAGE="$(job_image)"

if [[ -z "$CURRENT_JOB_IMAGE" ]]; then
  echo "error: unable to resolve current image for Cloud Run job: $MIGRATE_JOB" >&2
  exit 1
fi

log "Runtime service image: $(service_image)"
log "Current migrate job image: $CURRENT_JOB_IMAGE"
log "Target migrate job image: $IMAGE"

UPDATE_CMD=(
  gcloud run jobs update
)

run_job_update() {
  local eval_expression="$1"
  gcloud run jobs update "$MIGRATE_JOB" \
    --project "$GCP_PROJECT" \
    --region "$GCP_REGION" \
    --image "$IMAGE" \
    --command "bin/openagents_runtime" \
    --args "eval","$eval_expression" \
    --quiet
}

run_job_execute() {
  gcloud run jobs execute "$MIGRATE_JOB" \
    --project "$GCP_PROJECT" \
    --region "$GCP_REGION" \
    --wait \
    --format='value(metadata.name)'
}

extract_execution_name() {
  local output="$1"
  echo "$output" | grep -Eo "${MIGRATE_JOB}-[a-z0-9]+" | tail -n 1 || true
}

helper_missing_in_execution_logs() {
  local execution_name="$1"
  local attempt
  local match

  if [[ -z "$execution_name" ]]; then
    return 1
  fi

  for attempt in 1 2 3 4 5; do
    match="$(
      gcloud logging read "resource.type=\"cloud_run_job\" AND resource.labels.job_name=\"${MIGRATE_JOB}\" AND labels.\"run.googleapis.com/execution_name\"=\"${execution_name}\" AND textPayload:\"UndefinedFunctionError\" AND textPayload:\"migrate_and_verify\"" \
        --project "$GCP_PROJECT" \
        --limit=1 \
        --format='value(textPayload)' 2>/dev/null || true
    )"

    if [[ -n "$match" ]]; then
      return 0
    fi

    sleep 2
  done

  return 1
}

if [[ "$DRY_RUN" == "1" ]]; then
  log "DRY_RUN=1; planned commands:"
  printf '%q ' "${UPDATE_CMD[@]}" "$MIGRATE_JOB" \
    --project "$GCP_PROJECT" \
    --region "$GCP_REGION" \
    --image "$IMAGE" \
    --command "bin/openagents_runtime" \
    --args "eval","$MIGRATION_EVAL" \
    --quiet
  echo
  printf '%q ' gcloud run jobs execute "$MIGRATE_JOB" \
    --project "$GCP_PROJECT" \
    --region "$GCP_REGION" \
    --wait \
    --format='value(metadata.name)'
  echo
  exit 0
fi

run_job_update "$MIGRATION_EVAL"

UPDATED_JOB_IMAGE="$(job_image)"
if [[ "$UPDATED_JOB_IMAGE" != "$IMAGE" ]]; then
  echo "error: migrate job image mismatch after update (expected $IMAGE, got $UPDATED_JOB_IMAGE)" >&2
  exit 1
fi

set +e
EXECUTION_OUTPUT="$(run_job_execute 2>&1)"
EXECUTION_STATUS=$?
set -e

if [[ $EXECUTION_STATUS -ne 0 ]]; then
  FAILED_EXECUTION_NAME="$(extract_execution_name "$EXECUTION_OUTPUT")"

  if [[ "$MIGRATION_EVAL" == "$DEFAULT_MIGRATION_EVAL" ]] &&
    helper_missing_in_execution_logs "$FAILED_EXECUTION_NAME"; then
    log "Runtime image does not yet include migrate_and_verify!/0; falling back to migrate()/0 for this run."
    run_job_update "$FALLBACK_MIGRATION_EVAL"
    set +e
    EXECUTION_OUTPUT="$(run_job_execute 2>&1)"
    EXECUTION_STATUS=$?
    set -e
  fi
fi

if [[ $EXECUTION_STATUS -ne 0 ]]; then
  echo "$EXECUTION_OUTPUT" >&2
  FAILED_EXECUTION_NAME="$(extract_execution_name "$EXECUTION_OUTPUT")"
  if [[ -n "$FAILED_EXECUTION_NAME" ]]; then
    echo "Inspect failure logs:" >&2
    echo "gcloud logging read 'resource.type=\"cloud_run_job\" AND resource.labels.job_name=\"${MIGRATE_JOB}\" AND labels.\"run.googleapis.com/execution_name\"=\"${FAILED_EXECUTION_NAME}\"' --project \"${GCP_PROJECT}\" --limit=200 --format='table(timestamp,severity,textPayload)'" >&2
  fi
  exit $EXECUTION_STATUS
fi

EXECUTION_NAME="$(echo "$EXECUTION_OUTPUT" | tr -d '\r' | tail -n 1)"

if [[ -z "$EXECUTION_NAME" ]]; then
  echo "error: migration job execution name was empty" >&2
  exit 1
fi

log "Migration succeeded via execution: $EXECUTION_NAME"
log "Inspect execution logs:"
echo "gcloud logging read 'resource.type=\"cloud_run_job\" AND resource.labels.job_name=\"${MIGRATE_JOB}\" AND labels.\"run.googleapis.com/execution_name\"=\"${EXECUTION_NAME}\"' --project \"${GCP_PROJECT}\" --limit=200 --format='table(timestamp,severity,textPayload)'"
