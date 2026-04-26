#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
IDENTITY_SCRIPT="${SCRIPT_DIR}/17-provision-health-runner-identity.sh"
DEPLOY_SCRIPT="${SCRIPT_DIR}/18-deploy-health-runner-job.sh"
SMOKE_SCRIPT="${SCRIPT_DIR}/19-smoke-health-runner-job.sh"
SCHEDULER_SCRIPT="${SCRIPT_DIR}/20-deploy-health-runner-scheduler.sh"
COMMON_SCRIPT="${SCRIPT_DIR}/common.sh"
DOCKERFILE="${ROOT_DIR}/apps/nexus-relay/Dockerfile"

assert_contains() {
  local needle="$1"
  local haystack="$2"
  if ! grep -Fq -- "$needle" <<<"$haystack"; then
    printf 'missing expected content: %s\n' "$needle" >&2
    exit 1
  fi
}

assert_not_contains() {
  local needle="$1"
  local haystack="$2"
  if grep -Fq -- "$needle" <<<"$haystack"; then
    printf 'unexpected content found: %s\n' "$needle" >&2
    exit 1
  fi
}

bash -n "$IDENTITY_SCRIPT" "$DEPLOY_SCRIPT" "$SMOKE_SCRIPT" "$SCHEDULER_SCRIPT"

IDENTITY_TEXT="$(cat "$IDENTITY_SCRIPT")"
DEPLOY_TEXT="$(cat "$DEPLOY_SCRIPT")"
SMOKE_TEXT="$(cat "$SMOKE_SCRIPT")"
SCHEDULER_TEXT="$(cat "$SCHEDULER_SCRIPT")"
COMMON_TEXT="$(cat "$COMMON_SCRIPT")"
DOCKERFILE_TEXT="$(cat "$DOCKERFILE")"

assert_contains 'roles/secretmanager.secretAccessor' "$IDENTITY_TEXT"
assert_contains 'roles/logging.logWriter' "$IDENTITY_TEXT"
assert_contains 'roles/monitoring.metricWriter' "$IDENTITY_TEXT"
assert_contains 'roles/run.developer' "$IDENTITY_TEXT"
assert_contains 'NEXUS_HEALTH_RUNNER_SERVICE_ACCOUNT_EMAIL' "$IDENTITY_TEXT"
assert_contains '[redacted]' "$IDENTITY_TEXT"
assert_contains 'gcloud run jobs deploy' "$DEPLOY_TEXT"
assert_contains '--service-account' "$DEPLOY_TEXT"
assert_contains '--set-secrets' "$DEPLOY_TEXT"
assert_contains '/usr/local/bin/nexus-health-agent' "$DEPLOY_TEXT"
assert_contains 'NEXUS_HEALTH_AGENT_FORGE_BEARER_TOKEN' "$DEPLOY_TEXT"
assert_contains 'NEXUS_HEALTH_AGENT_FORGE_ACTOR_JWT' "$DEPLOY_TEXT"
assert_contains 'NEXUS_HEALTH_AGENT_NEXUS_ADMIN_BEARER_TOKEN' "$DEPLOY_TEXT"
assert_contains 'NEXUS_HEALTH_AGENT_EXTERNAL_VANTAGE_ID' "$DEPLOY_TEXT"
assert_contains 'NEXUS_HEALTH_AGENT_SCHEDULER_NAME' "$DEPLOY_TEXT"
assert_contains 'NEXUS_HEALTH_AGENT_SCHEDULER_INTERVAL_SECONDS' "$DEPLOY_TEXT"
assert_contains 'gcloud run jobs execute' "$SMOKE_TEXT"
assert_contains 'log secret scan result' "$SMOKE_TEXT"
assert_contains 'resource.labels.job_name' "$SMOKE_TEXT"
assert_contains 'gcloud scheduler jobs create http' "$SCHEDULER_TEXT"
assert_contains 'gcloud scheduler jobs update http' "$SCHEDULER_TEXT"
assert_contains '--oauth-service-account-email' "$SCHEDULER_TEXT"
assert_contains ':run' "$SCHEDULER_TEXT"
assert_contains 'cloudscheduler.googleapis.com' "$COMMON_TEXT"
assert_contains 'nexus-health-agent' "$DOCKERFILE_TEXT"

for target_text in "$IDENTITY_TEXT" "$DEPLOY_TEXT" "$SMOKE_TEXT" "$SCHEDULER_TEXT"; do
  assert_not_contains 'gcloud auth login' "$target_text"
  assert_not_contains 'iam service-accounts keys create' "$target_text"
done

IDENTITY_OUTPUT="$(
  NEXUS_HEALTH_RUNNER_DRY_RUN=true \
  NEXUS_HEALTH_RUNNER_SECRET_SMOKE_ENABLED=true \
  bash "$IDENTITY_SCRIPT" 2>&1
)"
assert_contains 'gcloud iam service-accounts create' "$IDENTITY_OUTPUT"
assert_contains 'gcloud secrets add-iam-policy-binding' "$IDENTITY_OUTPUT"
assert_contains 'roles/secretmanager.secretAccessor' "$IDENTITY_OUTPUT"
assert_contains 'secret access smoke output' "$IDENTITY_OUTPUT"
assert_contains '[redacted]' "$IDENTITY_OUTPUT"

DEPLOY_OUTPUT="$(
  NEXUS_HEALTH_RUNNER_DRY_RUN=true \
  NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS=false \
  NEXUS_HEALTH_RUNNER_JOB_ARGS='--dry-run,--json' \
  bash "$DEPLOY_SCRIPT" 2>&1
)"
assert_contains 'gcloud run jobs deploy' "$DEPLOY_OUTPUT"
assert_contains '--service-account' "$DEPLOY_OUTPUT"
assert_contains '--command=/usr/local/bin/nexus-health-agent' "$DEPLOY_OUTPUT"
assert_contains '--args=--dry-run\,--json' "$DEPLOY_OUTPUT"
assert_not_contains 'NEXUS_HEALTH_AGENT_FORGE_BEARER_TOKEN=' "$DEPLOY_OUTPUT"

ADMIN_DEPLOY_OUTPUT="$(
  NEXUS_HEALTH_RUNNER_DRY_RUN=true \
  NEXUS_HEALTH_RUNNER_ATTACH_FORGE_SECRETS=false \
  NEXUS_HEALTH_RUNNER_ATTACH_NEXUS_ADMIN_SECRET=true \
  NEXUS_HEALTH_RUNNER_JOB_ARGS='--action-kind,treasury_refresh,--forge-lease-id,forge-lease-dry-run,--dry-run,--json' \
  bash "$DEPLOY_SCRIPT" 2>&1
)"
assert_contains 'NEXUS_HEALTH_AGENT_NEXUS_ADMIN_BEARER_TOKEN=' "$ADMIN_DEPLOY_OUTPUT"
assert_contains '--action-kind\,treasury_refresh' "$ADMIN_DEPLOY_OUTPUT"

SMOKE_OUTPUT="$(
  NEXUS_HEALTH_RUNNER_DRY_RUN=true \
  bash "$SMOKE_SCRIPT" 2>&1
)"
assert_contains 'gcloud run jobs execute' "$SMOKE_OUTPUT"
assert_contains 'log secret scan result: passed' "$SMOKE_OUTPUT"
assert_contains '[redacted]' "$SMOKE_OUTPUT"

SCHEDULER_OUTPUT="$(
  NEXUS_HEALTH_RUNNER_DRY_RUN=true \
  bash "$SCHEDULER_SCRIPT" 2>&1
)"
assert_contains 'gcloud scheduler jobs create http' "$SCHEDULER_OUTPUT"
assert_contains 'gcloud scheduler jobs update http' "$SCHEDULER_OUTPUT"
assert_contains '--schedule' "$SCHEDULER_OUTPUT"
assert_contains 'run.googleapis.com' "$SCHEDULER_OUTPUT"
assert_contains ':run' "$SCHEDULER_OUTPUT"

printf 'ok: nexus health-runner deploy lane has hosted-identity and redaction guards\n'
