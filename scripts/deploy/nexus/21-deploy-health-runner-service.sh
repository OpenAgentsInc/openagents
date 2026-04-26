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

gcloud_delimited_join() {
  local delimiter='|'
  local item

  printf '^%s^' "$delimiter"
  local first=true
  for item in "$@"; do
    [[ "$item" != *"$delimiter"* ]] || die "gcloud env value cannot contain delimiter '${delimiter}': ${item%%=*}"
    if [[ "$first" == "true" ]]; then
      first=false
    else
      printf '%s' "$delimiter"
    fi
    printf '%s' "$item"
  done
}

server_args_contain() {
  local needle="$1"
  [[ ",${NEXUS_HEALTH_RUNNER_SERVER_ARGS}," == *",${needle},"* ]]
}

validate_runner_config() {
  if ! server_args_contain "--dry-run" && ! server_args_contain "--fake-forge"; then
    [[ -n "$NEXUS_HEALTH_RUNNER_FORGE_BASE_URL" ]] || die "NEXUS_HEALTH_RUNNER_FORGE_BASE_URL is required for non-dry-run health runner services"
  fi

  if [[ "$NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS" == "true" ]]; then
    if server_args_contain "--dry-run" || server_args_contain "--fake-forge"; then
      log "Forge secrets are still attached even though the service args do not require live Forge writes"
      log "Set NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS=false for a public-read-only monitor service"
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
  "NEXUS_HEALTH_AGENT_SERVER_ARGS=${NEXUS_HEALTH_RUNNER_SERVER_ARGS}"
)

if [[ -n "$NEXUS_HEALTH_RUNNER_FORGE_BASE_URL" ]]; then
  ENV_VARS+=("NEXUS_HEALTH_AGENT_FORGE_BASE_URL=${NEXUS_HEALTH_RUNNER_FORGE_BASE_URL}")
fi

CMD=(
  gcloud run deploy "$NEXUS_HEALTH_RUNNER_SERVICE"
  --project "$GCP_PROJECT"
  --region "$NEXUS_HEALTH_RUNNER_REGION"
  --image "$NEXUS_HEALTH_RUNNER_IMAGE"
  --service-account "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL"
  --command="/usr/local/bin/nexus-health-agent-server"
  --cpu "$NEXUS_HEALTH_RUNNER_CPU"
  --memory "$NEXUS_HEALTH_RUNNER_MEMORY"
  --min-instances "$NEXUS_HEALTH_RUNNER_SERVICE_MIN_INSTANCES"
  --max-instances "$NEXUS_HEALTH_RUNNER_SERVICE_MAX_INSTANCES"
  --set-env-vars "$(gcloud_delimited_join "${ENV_VARS[@]}")"
  --no-allow-unauthenticated
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
  CMD+=(--set-secrets "$(gcloud_delimited_join "${SECRET_VARS[@]}")")
fi

if dry_run; then
  log "Dry-run enabled; printing Cloud Run Service deploy command without executing it"
  show_cmd "${CMD[@]}"
  show_cmd gcloud run services describe "$NEXUS_HEALTH_RUNNER_SERVICE" \
    --project "$GCP_PROJECT" \
    --region "$NEXUS_HEALTH_RUNNER_REGION" \
    --format "value(status.url)"
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

SERVICE_URL="$(gcloud run services describe "$NEXUS_HEALTH_RUNNER_SERVICE" \
  --project "$GCP_PROJECT" \
  --region "$NEXUS_HEALTH_RUNNER_REGION" \
  --format "value(status.url)")"

gcloud run services add-iam-policy-binding "$NEXUS_HEALTH_RUNNER_SERVICE" \
  --project "$GCP_PROJECT" \
  --region "$NEXUS_HEALTH_RUNNER_REGION" \
  --member "serviceAccount:${NEXUS_HEALTH_RUNNER_SCHEDULER_OAUTH_SERVICE_ACCOUNT_EMAIL}" \
  --role roles/run.invoker \
  --quiet >/dev/null

log "Nexus health runner service deployed: ${NEXUS_HEALTH_RUNNER_SERVICE}"
log "Image: ${NEXUS_HEALTH_RUNNER_IMAGE}"
log "Service account: ${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL}"
log "URL: ${SERVICE_URL}"
