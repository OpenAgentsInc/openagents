#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

RECOVERY_PERMISSIONS=(
  compute.instances.get
  compute.instances.list
  compute.instances.reset
  compute.instances.getSerialPortOutput
  compute.zoneOperations.get
  compute.zones.get
)

dry_run() {
  [[ "$NEXUS_RECOVERY_IDENTITY_DRY_RUN" == "true" ]]
}

show_cmd() {
  printf '[nexus-recovery-identity-dry-run] %q' "$1"
  shift
  local arg
  for arg in "$@"; do
    printf ' %q' "$arg"
  done
  printf '\n'
}

run_or_show() {
  if dry_run; then
    show_cmd "$@"
  else
    "$@"
  fi
}

run_or_show_quiet() {
  if dry_run; then
    show_cmd "$@"
  else
    "$@" >/dev/null
  fi
}

permission_csv() {
  local IFS=,
  printf '%s' "${RECOVERY_PERMISSIONS[*]}"
}

custom_role_name() {
  printf 'projects/%s/roles/%s' "$GCP_PROJECT" "$NEXUS_RECOVERY_CUSTOM_ROLE_ID"
}

ensure_service_account() {
  if dry_run; then
    show_cmd gcloud iam service-accounts describe "$NEXUS_RECOVERY_SERVICE_ACCOUNT_EMAIL" \
      --project "$GCP_PROJECT"
    show_cmd gcloud iam service-accounts create "$NEXUS_RECOVERY_SERVICE_ACCOUNT_NAME" \
      --project "$GCP_PROJECT" \
      --display-name "OpenAgents Nexus recovery operator" \
      --description "Least-privilege Nexus VM audit, watchdog repair, reset, and binary hotfix identity"
    return
  fi

  if ! gcloud iam service-accounts describe "$NEXUS_RECOVERY_SERVICE_ACCOUNT_EMAIL" \
    --project "$GCP_PROJECT" >/dev/null 2>&1; then
    log "Creating service account: ${NEXUS_RECOVERY_SERVICE_ACCOUNT_EMAIL}"
    gcloud iam service-accounts create "$NEXUS_RECOVERY_SERVICE_ACCOUNT_NAME" \
      --project "$GCP_PROJECT" \
      --display-name "OpenAgents Nexus recovery operator" \
      --description "Least-privilege Nexus VM audit, watchdog repair, reset, and binary hotfix identity" >/dev/null
  fi
}

ensure_custom_role() {
  local role_id="$NEXUS_RECOVERY_CUSTOM_ROLE_ID"
  local permissions
  permissions="$(permission_csv)"

  if dry_run; then
    show_cmd gcloud iam roles describe "$role_id" --project "$GCP_PROJECT"
    show_cmd gcloud iam roles create "$role_id" \
      --project "$GCP_PROJECT" \
      --title "Nexus Recovery Operator" \
      --description "Audit and recover Nexus VM public-edge outages without project-owner credentials" \
      --permissions "$permissions" \
      --stage GA
    show_cmd gcloud iam roles update "$role_id" \
      --project "$GCP_PROJECT" \
      --title "Nexus Recovery Operator" \
      --description "Audit and recover Nexus VM public-edge outages without project-owner credentials" \
      --permissions "$permissions" \
      --stage GA
    return
  fi

  if gcloud iam roles describe "$role_id" --project "$GCP_PROJECT" >/dev/null 2>&1; then
    log "Updating custom role: $(custom_role_name)"
    gcloud iam roles update "$role_id" \
      --project "$GCP_PROJECT" \
      --title "Nexus Recovery Operator" \
      --description "Audit and recover Nexus VM public-edge outages without project-owner credentials" \
      --permissions "$permissions" \
      --stage GA >/dev/null
  else
    log "Creating custom role: $(custom_role_name)"
    gcloud iam roles create "$role_id" \
      --project "$GCP_PROJECT" \
      --title "Nexus Recovery Operator" \
      --description "Audit and recover Nexus VM public-edge outages without project-owner credentials" \
      --permissions "$permissions" \
      --stage GA >/dev/null
  fi
}

grant_project_role() {
  local role="$1"
  run_or_show_quiet gcloud projects add-iam-policy-binding "$GCP_PROJECT" \
    --member "serviceAccount:${NEXUS_RECOVERY_SERVICE_ACCOUNT_EMAIL}" \
    --role "$role"
}

verify_project_role() {
  local role="$1"
  local member="serviceAccount:${NEXUS_RECOVERY_SERVICE_ACCOUNT_EMAIL}"

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

grant_impersonator_if_requested() {
  if [[ -z "$NEXUS_RECOVERY_IMPERSONATOR_MEMBER" ]]; then
    log "No recovery impersonator requested; set NEXUS_RECOVERY_IMPERSONATOR_MEMBER to grant Token Creator on ${NEXUS_RECOVERY_SERVICE_ACCOUNT_EMAIL}"
    return
  fi

  run_or_show_quiet gcloud iam service-accounts add-iam-policy-binding \
    "$NEXUS_RECOVERY_SERVICE_ACCOUNT_EMAIL" \
    --project "$GCP_PROJECT" \
    --member "$NEXUS_RECOVERY_IMPERSONATOR_MEMBER" \
    --role roles/iam.serviceAccountTokenCreator
}

if ! dry_run; then
  require_cmd gcloud
  ensure_gcloud_context
  ensure_services
else
  log "Dry-run enabled; printing Nexus recovery identity commands without executing them"
fi

ensure_service_account
ensure_custom_role

for role in \
  "$(custom_role_name)" \
  roles/iap.tunnelResourceAccessor \
  roles/compute.osAdminLogin; do
  grant_project_role "$role"
  verify_project_role "$role"
done

grant_impersonator_if_requested

log "Nexus recovery identity ready: ${NEXUS_RECOVERY_SERVICE_ACCOUNT_EMAIL}"
log "Use with: CLOUDSDK_AUTH_IMPERSONATE_SERVICE_ACCOUNT=${NEXUS_RECOVERY_SERVICE_ACCOUNT_EMAIL} scripts/deploy/nexus/33-audit-public-watchdog.sh"
