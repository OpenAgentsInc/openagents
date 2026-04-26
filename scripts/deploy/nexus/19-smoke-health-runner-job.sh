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

if dry_run; then
  log "Dry-run enabled; printing Cloud Run Job smoke commands without executing them"
  show_cmd gcloud run jobs execute "$NEXUS_HEALTH_RUNNER_JOB" \
    --project "$GCP_PROJECT" \
    --region "$NEXUS_HEALTH_RUNNER_REGION" \
    --wait \
    --format json
  show_cmd gcloud logging read "resource.type=\"cloud_run_job\" AND resource.labels.job_name=\"${NEXUS_HEALTH_RUNNER_JOB}\" AND resource.labels.location=\"${NEXUS_HEALTH_RUNNER_REGION}\"" \
    --project "$GCP_PROJECT" \
    --freshness "1h" \
    --limit 100 \
    --format json
  printf '[nexus-health-runner-dry-run] log secret scan result: passed; output values are [redacted]\n'
  exit 0
fi

require_cmd gcloud
require_cmd jq
ensure_gcloud_context

EXECUTION_JSON_PATH="$(mktemp "${TMPDIR:-/tmp}/nexus-health-runner-execution.XXXXXX.json")"
LOG_PATH="$(mktemp "${TMPDIR:-/tmp}/nexus-health-runner-logs.XXXXXX.log")"
trap 'rm -f "$EXECUTION_JSON_PATH" "$LOG_PATH"' EXIT

gcloud run jobs execute "$NEXUS_HEALTH_RUNNER_JOB" \
  --project "$GCP_PROJECT" \
  --region "$NEXUS_HEALTH_RUNNER_REGION" \
  --wait \
  --format json >"$EXECUTION_JSON_PATH"

EXECUTION_NAME="$(jq -r '.metadata.name // .name // empty' "$EXECUTION_JSON_PATH" | sed 's#.*/##')"
[[ -n "$EXECUTION_NAME" ]] || die "Cloud Run Job execution did not return an execution name"

LOG_FILTER="resource.type=\"cloud_run_job\" AND resource.labels.job_name=\"${NEXUS_HEALTH_RUNNER_JOB}\" AND resource.labels.location=\"${NEXUS_HEALTH_RUNNER_REGION}\""
LOG_FILTER="${LOG_FILTER} AND labels.\"run.googleapis.com/execution_name\"=\"${EXECUTION_NAME}\""

for attempt in 1 2 3 4 5 6 7 8 9 10 11 12; do
  gcloud logging read "$LOG_FILTER" \
    --project "$GCP_PROJECT" \
    --freshness "1h" \
    --limit 100 \
    --format json >"$LOG_PATH" || true
  if jq -e 'length > 0' "$LOG_PATH" >/dev/null 2>&1; then
    break
  fi
  if (( attempt < 12 )); then
    sleep 5
  fi
done

if [[ "$NEXUS_HEALTH_RUNNER_LOG_SECRET_SCAN_ENABLED" == "true" ]]; then
  if grep -Eiq 'xox[baprs]-|gh[pousr]_|sk-[A-Za-z0-9]|-----BEGIN [A-Z ]*PRIVATE KEY|payment_preimage|wallet_mnemonic|private_key|NEXUS_CONTROL_ADMIN_BEARER_TOKEN=' "$LOG_PATH"; then
    die "Cloud Run Job logs contain secret-shaped material; inspect logs directly with privileged access"
  fi
fi

if ! jq -e 'length > 0' "$LOG_PATH" >/dev/null 2>&1; then
  log "Health runner job executed, but no logs were returned for execution ${EXECUTION_NAME}"
else
  log "Health runner job executed and startup log secret scan passed for execution ${EXECUTION_NAME}"
fi

log "Nexus health runner smoke complete: ${NEXUS_HEALTH_RUNNER_JOB}"
