#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

SECRETS=(
  "$NEXUS_HEALTH_RUNNER_SECRET_FORGE_BEARER_TOKEN"
  "$NEXUS_HEALTH_RUNNER_SECRET_FORGE_ACTOR_JWT"
  "$NEXUS_HEALTH_RUNNER_SECRET_NEXUS_ADMIN_BEARER_TOKEN"
)

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

run_or_show_quiet() {
  if dry_run; then
    show_cmd "$@"
  else
    "$@" >/dev/null
  fi
}

run_or_show_quiet_with_retry() {
  if dry_run; then
    show_cmd "$@"
    return
  fi

  local attempts=8
  local delay_seconds=5
  local attempt=1
  until "$@" >/dev/null; do
    if (( attempt >= attempts )); then
      return 1
    fi
    log "Command failed while IAM may still be propagating; retrying attempt $((attempt + 1))/${attempts}"
    sleep "$delay_seconds"
    attempt=$((attempt + 1))
  done
}

wait_for_service_account() {
  if dry_run; then
    show_cmd gcloud iam service-accounts describe "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL" \
      --project "$GCP_PROJECT"
    return
  fi

  local attempts=12
  local delay_seconds=5
  local attempt=1
  until gcloud iam service-accounts describe "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL" \
    --project "$GCP_PROJECT" >/dev/null 2>&1; do
    if (( attempt >= attempts )); then
      die "Service account did not become visible to IAM: ${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL}"
    fi
    log "Waiting for service account propagation: ${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL} attempt $((attempt + 1))/${attempts}"
    sleep "$delay_seconds"
    attempt=$((attempt + 1))
  done
}

ensure_secret_exists() {
  local secret_name="$1"
  if dry_run; then
    show_cmd gcloud secrets describe "$secret_name" --project "$GCP_PROJECT"
    show_cmd gcloud secrets create "$secret_name" \
      --project "$GCP_PROJECT" \
      --replication-policy automatic \
      --labels "app=openagents,component=nexus-health-runner"
    return
  fi

  if ! gcloud secrets describe "$secret_name" --project "$GCP_PROJECT" >/dev/null 2>&1; then
    log "Creating Secret Manager secret: ${secret_name}"
    gcloud secrets create "$secret_name" \
      --project "$GCP_PROJECT" \
      --replication-policy automatic \
      --labels "app=openagents,component=nexus-health-runner" >/dev/null
  fi
}

grant_secret_access() {
  local secret_name="$1"
  run_or_show_quiet_with_retry gcloud secrets add-iam-policy-binding "$secret_name" \
    --project "$GCP_PROJECT" \
    --member "serviceAccount:${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL}" \
    --role roles/secretmanager.secretAccessor
}

verify_project_role() {
  local role="$1"
  local member="serviceAccount:${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL}"

  if dry_run; then
    show_cmd gcloud projects get-iam-policy "$GCP_PROJECT" \
      --flatten "bindings[].members" \
      --filter "bindings.role=${role} AND bindings.members=${member}" \
      --format "value(bindings.role)"
    return
  fi

  local matched_role
  matched_role="$(gcloud projects get-iam-policy "$GCP_PROJECT" \
    --flatten "bindings[].members" \
    --filter "bindings.role=${role} AND bindings.members=${member}" \
    --format "value(bindings.role)" | head -n 1)"
  [[ "$matched_role" == "$role" ]] || die "IAM verification failed for ${member} ${role}"
}

verify_secret_access() {
  local secret_name="$1"
  local member="serviceAccount:${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL}"

  if dry_run; then
    show_cmd gcloud secrets get-iam-policy "$secret_name" \
      --project "$GCP_PROJECT" \
      --flatten "bindings[].members" \
      --filter "bindings.role=roles/secretmanager.secretAccessor AND bindings.members=${member}" \
      --format "value(bindings.role)"
    return
  fi

  local matched_role
  matched_role="$(gcloud secrets get-iam-policy "$secret_name" \
    --project "$GCP_PROJECT" \
    --flatten "bindings[].members" \
    --filter "bindings.role=roles/secretmanager.secretAccessor AND bindings.members=${member}" \
    --format "value(bindings.role)" | head -n 1)"
  [[ "$matched_role" == "roles/secretmanager.secretAccessor" ]] || die "Secret IAM verification failed for ${secret_name}"
}

secret_access_smoke() {
  local secret_name="$1"
  if [[ "$NEXUS_HEALTH_RUNNER_SECRET_SMOKE_ENABLED" != "true" ]]; then
    return
  fi
  if dry_run; then
    show_cmd gcloud secrets versions access latest \
      --project "$GCP_PROJECT" \
      --secret "$secret_name" \
      --impersonate-service-account "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL"
    printf '[nexus-health-runner-dry-run] secret access smoke output for %s: [redacted]\n' "$secret_name"
    return
  fi

  gcloud secrets versions access latest \
    --project "$GCP_PROJECT" \
    --secret "$secret_name" \
    --impersonate-service-account "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL" >/dev/null
  log "Secret access smoke succeeded for ${secret_name}: [redacted]"
}

if ! dry_run; then
  require_cmd gcloud
  ensure_gcloud_context
  ensure_services
else
  log "Dry-run enabled; printing GCP identity and Secret Manager commands without executing them"
fi

if dry_run; then
  show_cmd gcloud iam service-accounts describe "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL" \
    --project "$GCP_PROJECT"
  show_cmd gcloud iam service-accounts create "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_NAME" \
    --project "$GCP_PROJECT" \
    --display-name "OpenAgents Nexus health runner" \
    --description "Runs monitor-only OpenAgents Nexus health probes from hosted GCP identity"
elif ! gcloud iam service-accounts describe "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL" \
  --project "$GCP_PROJECT" >/dev/null 2>&1; then
  log "Creating service account: ${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL}"
  gcloud iam service-accounts create "$NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_NAME" \
    --project "$GCP_PROJECT" \
    --display-name "OpenAgents Nexus health runner" \
    --description "Runs monitor-only OpenAgents Nexus health probes from hosted GCP identity" >/dev/null
fi

wait_for_service_account

for role in roles/logging.logWriter roles/monitoring.metricWriter; do
  run_or_show_quiet_with_retry gcloud projects add-iam-policy-binding "$GCP_PROJECT" \
    --member "serviceAccount:${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL}" \
    --role "$role"
  verify_project_role "$role"
done

for secret_name in "${SECRETS[@]}"; do
  ensure_secret_exists "$secret_name"
  grant_secret_access "$secret_name"
  verify_secret_access "$secret_name"
  secret_access_smoke "$secret_name"
done

log "Nexus health runner identity ready: ${NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL}"
log "Secret Manager bindings ready for ${#SECRETS[@]} secret(s); values were not printed"
