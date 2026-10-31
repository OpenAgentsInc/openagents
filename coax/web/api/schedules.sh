#!/usr/bin/env bash
# Puts schedules.conf into Cloud Scheduler: one job per line and service, each
# calling POST https://<host>/api/jobs/<job> in America/Chicago (the scheduler
# keeps daylight saving) with an OIDC token for oa-api-jobs, which jobs.c
# accepts (OA_JOBS_CALLER, set by deploy.sh). It prints each job's next run.
#   coax/web/api/schedules.sh [--staging]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
project=openagentsgemini region=us-central1 caller="oa-api-jobs@openagentsgemini.iam.gserviceaccount.com"
hosts=(staging.openagents.com openagents.com); prefixes=(oa-api-staging oa-api)
[[ "${1:-}" == --staging ]] && hosts=(staging.openagents.com) prefixes=(oa-api-staging)
grep -v '^#' "$here/schedules.conf" | while read -r job min hour dom mon dow; do
  [[ -n "$job" ]] || continue
  for i in "${!hosts[@]}"; do
    name="${prefixes[$i]}-$job" args=(--project "$project" --location "$region" --schedule "$min $hour $dom $mon $dow"
      --time-zone America/Chicago --uri "https://${hosts[$i]}/api/jobs/$job" --http-method POST
      --oidc-service-account-email "$caller" --oidc-token-audience "https://${hosts[$i]}/api/jobs" --attempt-deadline 300s)
    if gcloud scheduler jobs describe "$name" --project "$project" --location "$region" >/dev/null 2>&1; then
      gcloud scheduler jobs update http "$name" "${args[@]}" --quiet >/dev/null
    else
      gcloud scheduler jobs create http "$name" "${args[@]}" --quiet >/dev/null
    fi
    echo "$name: next $(gcloud scheduler jobs describe "$name" --project "$project" --location "$region" --format 'value(scheduleTime)')"
  done
done
