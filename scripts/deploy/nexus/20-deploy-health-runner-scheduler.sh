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

scheduler_base_cmd() {
  "$@" \
    "$NEXUS_HEALTH_RUNNER_SCHEDULER_NAME" \
    --project "$GCP_PROJECT" \
    --location "$NEXUS_HEALTH_RUNNER_REGION" \
    --schedule "$NEXUS_HEALTH_RUNNER_SCHEDULER_CRON" \
    --time-zone "$NEXUS_HEALTH_RUNNER_SCHEDULER_TIME_ZONE" \
    --description "$NEXUS_HEALTH_RUNNER_SCHEDULER_DESCRIPTION" \
    --uri "$NEXUS_HEALTH_RUNNER_SCHEDULER_URI" \
    --http-method POST \
    --oauth-service-account-email "$NEXUS_HEALTH_RUNNER_SCHEDULER_OAUTH_SERVICE_ACCOUNT_EMAIL" \
    --oauth-token-scope "https://www.googleapis.com/auth/cloud-platform" \
    --message-body "{}" \
    --quiet
}

validate_scheduler_config() {
  [[ -n "$NEXUS_HEALTH_RUNNER_SCHEDULER_NAME" ]] || die "NEXUS_HEALTH_RUNNER_SCHEDULER_NAME is required"
  [[ -n "$NEXUS_HEALTH_RUNNER_SCHEDULER_CRON" ]] || die "NEXUS_HEALTH_RUNNER_SCHEDULER_CRON is required"
  [[ -n "$NEXUS_HEALTH_RUNNER_SCHEDULER_URI" ]] || die "NEXUS_HEALTH_RUNNER_SCHEDULER_URI is required"
  [[ "$NEXUS_HEALTH_RUNNER_SCHEDULER_URI" == https://*run.googleapis.com/*":run" ]] || die "scheduler URI must target the Cloud Run Jobs run endpoint"
  [[ -n "$NEXUS_HEALTH_RUNNER_SCHEDULER_OAUTH_SERVICE_ACCOUNT_EMAIL" ]] || die "NEXUS_HEALTH_RUNNER_SCHEDULER_OAUTH_SERVICE_ACCOUNT_EMAIL is required"
}

validate_scheduler_config

if dry_run; then
  log "Dry-run enabled; printing Cloud Scheduler commands without executing them"
  show_cmd gcloud scheduler jobs describe "$NEXUS_HEALTH_RUNNER_SCHEDULER_NAME" \
    --project "$GCP_PROJECT" \
    --location "$NEXUS_HEALTH_RUNNER_REGION" \
    --format json
  scheduler_base_cmd show_cmd gcloud scheduler jobs create http
  scheduler_base_cmd show_cmd gcloud scheduler jobs update http
  exit 0
fi

require_cmd gcloud
ensure_gcloud_context
ensure_services

gcloud iam service-accounts describe "$NEXUS_HEALTH_RUNNER_SCHEDULER_OAUTH_SERVICE_ACCOUNT_EMAIL" \
  --project "$GCP_PROJECT" >/dev/null
gcloud run jobs describe "$NEXUS_HEALTH_RUNNER_JOB" \
  --project "$GCP_PROJECT" \
  --region "$NEXUS_HEALTH_RUNNER_REGION" >/dev/null

if gcloud scheduler jobs describe "$NEXUS_HEALTH_RUNNER_SCHEDULER_NAME" \
  --project "$GCP_PROJECT" \
  --location "$NEXUS_HEALTH_RUNNER_REGION" >/dev/null 2>&1; then
  scheduler_base_cmd gcloud scheduler jobs update http
else
  scheduler_base_cmd gcloud scheduler jobs create http
fi

log "Nexus health runner scheduler ready: ${NEXUS_HEALTH_RUNNER_SCHEDULER_NAME}"
log "Schedule: ${NEXUS_HEALTH_RUNNER_SCHEDULER_CRON} (${NEXUS_HEALTH_RUNNER_SCHEDULER_TIME_ZONE})"
log "Target: ${NEXUS_HEALTH_RUNNER_SCHEDULER_URI}"
