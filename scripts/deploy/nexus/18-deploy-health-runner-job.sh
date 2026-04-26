#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

dry_run() {
  [[ "$NEXUS_HEALTH_RUNNER_DRY_RUN" == "true" ]]
}

show_cmd() {
  printf '[nexus-health-runner-dry-run] %q' "$1"
  shift
  local arg
  for arg in "$@"; do
    printf ' %q' "$arg"
  done
  printf '\n'
}

csv_join() {
  local IFS=,
  printf '%s' "$*"
}

job_args_contain() {
  local needle="$1"
  [[ ",${NEXUS_HEALTH_RUNNER_JOB_ARGS}," == *",${needle},"* ]]
}

validate_runner_config() {
  if ! job_args_contain "--dry-run" && ! job_args_contain "--fake-forge"; then
    [[ -n "$NEXUS_HEALTH_RUNNER_FORGE_BASE_URL" ]] || die "NEXUS_HEALTH_RUNNER_FORGE_BASE_URL is required for non-dry-run health runner jobs"
  fi

  if [[ "$NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS" == "true" ]]; then
    if job_args_contain "--dry-run" || job_args_contain "--fake-forge"; then
      log "Forge secrets are still attached even though the job args do not require live Forge writes"
      log "Set NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS=false for a public-read-only smoke job"
    fi
  fi
}

validate_runner_config

ENV_VARS=(
  "NEXUS_HEALTH_AGENT_NEXUS_BASE_URL=${NEXUS_HEALTH_RUNNER_NEXUS_BASE_URL}"
  "NEXUS_HEALTH_AGENT_PROJECT_ID=${NEXUS_HEALTH_RUNNER_PROJECT_ID}"
  "NEXUS_HEALTH_AGENT_ACTOR_ID=${NEXUS_HEALTH_RUNNER_ACTOR_ID}"
  "NEXUS_HEALTH_AGENT_EXTERNAL_VANTAGE_ID=${NEXUS_HEALTH_RUNNER_EXTERNAL_VANTAGE_ID}"
  "NEXUS_HEALTH_AGENT_SCHEDULER_NAME=${NEXUS_HEALTH_RUNNER_SCHEDULER_NAME}"
  "NEXUS_HEALTH_AGENT_SCHEDULER_INTERVAL_SECONDS=${NEXUS_HEALTH_RUNNER_SCHEDULER_INTERVAL_SECONDS}"
)

if [[ -n "$NEXUS_HEALTH_RUNNER_FORGE_BASE_URL" ]]; then
  ENV_VARS+=("NEXUS_HEALTH_AGENT_FORGE_BASE_URL=${NEXUS_HEALTH_RUNNER_FORGE_BASE_URL}")
fi

CMD=(
  gcloud run jobs deploy "$NEXUS_HEALTH_RUNNER_JOB"
  --project "$GCP_PROJECT"
  --region "$NEXUS_HEALTH_RUNNER_REGION"
  --image "$NEXUS_HEALTH_RUNNER_IMAGE"
  --service-account "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL"
  --command="/usr/local/bin/nexus-health-agent"
  --args="${NEXUS_HEALTH_RUNNER_JOB_ARGS}"
  --cpu "$NEXUS_HEALTH_RUNNER_CPU"
  --memory "$NEXUS_HEALTH_RUNNER_MEMORY"
  --task-timeout "$NEXUS_HEALTH_RUNNER_TASK_TIMEOUT"
  --max-retries "$NEXUS_HEALTH_RUNNER_MAX_RETRIES"
  --set-env-vars "$(csv_join "${ENV_VARS[@]}")"
  --quiet
)
SECRET_VARS=()

if [[ "$NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS" == "true" ]]; then
  SECRET_VARS+=(
    "NEXUS_HEALTH_AGENT_FORGE_BEARER_TOKEN=${NEXUS_HEALTH_RUNNER_SECRET_FORGE_BEARER_TOKEN}:latest"
    "NEXUS_HEALTH_AGENT_FORGE_ACTOR_JWT=${NEXUS_HEALTH_RUNNER_SECRET_FORGE_ACTOR_JWT}:latest"
  )
fi

if [[ "$NEXUS_HEALTH_RUNNER_ATTACH_NEXUS_ADMIN_SECRET" == "true" ]]; then
  SECRET_VARS+=(
    "NEXUS_HEALTH_AGENT_NEXUS_ADMIN_BEARER_TOKEN=${NEXUS_HEALTH_RUNNER_SECRET_NEXUS_ADMIN_BEARER_TOKEN}:latest"
  )
fi

if (( ${#SECRET_VARS[@]} > 0 )); then
  CMD+=(--set-secrets "$(csv_join "${SECRET_VARS[@]}")")
fi

if dry_run; then
  log "Dry-run enabled; printing Cloud Run Job deploy command without executing it"
  show_cmd "${CMD[@]}"
  show_cmd gcloud run jobs describe "$NEXUS_HEALTH_RUNNER_JOB" \
    --project "$GCP_PROJECT" \
    --region "$NEXUS_HEALTH_RUNNER_REGION" \
    --format "value(spec.template.spec.template.spec.serviceAccountName)"
  exit 0
fi

require_cmd gcloud
ensure_gcloud_context
ensure_services

gcloud iam service-accounts describe "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL" \
  --project "$GCP_PROJECT" >/dev/null

if [[ "$NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS" == "true" ]]; then
  gcloud secrets describe "$NEXUS_HEALTH_RUNNER_SECRET_FORGE_BEARER_TOKEN" \
    --project "$GCP_PROJECT" >/dev/null
  gcloud secrets describe "$NEXUS_HEALTH_RUNNER_SECRET_FORGE_ACTOR_JWT" \
    --project "$GCP_PROJECT" >/dev/null
fi

if [[ "$NEXUS_HEALTH_RUNNER_ATTACH_NEXUS_ADMIN_SECRET" == "true" ]]; then
  gcloud secrets describe "$NEXUS_HEALTH_RUNNER_SECRET_NEXUS_ADMIN_BEARER_TOKEN" \
    --project "$GCP_PROJECT" >/dev/null
fi

"${CMD[@]}"

DEPLOYED_SERVICE_ACCOUNT="$(gcloud run jobs describe "$NEXUS_HEALTH_RUNNER_JOB" \
  --project "$GCP_PROJECT" \
  --region "$NEXUS_HEALTH_RUNNER_REGION" \
  --format "value(spec.template.spec.template.spec.serviceAccountName)")"

case "$DEPLOYED_SERVICE_ACCOUNT" in
  "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL"|*"serviceAccounts/${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL}")
    ;;
  *)
    die "Cloud Run Job service account mismatch: expected ${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL}, got ${DEPLOYED_SERVICE_ACCOUNT:-<empty>}"
    ;;
esac

log "Nexus health runner job deployed: ${NEXUS_HEALTH_RUNNER_JOB}"
log "Image: ${NEXUS_HEALTH_RUNNER_IMAGE}"
log "Service account: ${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL}"
