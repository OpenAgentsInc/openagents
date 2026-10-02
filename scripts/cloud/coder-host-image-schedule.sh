#!/usr/bin/env bash
#
# Create or update the daily `oa-coder-host` image bake.
#
#   scripts/cloud/coder-host-image-schedule.sh            # print the job body
#   scripts/cloud/coder-host-image-schedule.sh --apply    # create or update the job
#   scripts/cloud/coder-host-image-schedule.sh --run-now  # trigger one bake now
#
# Cloud Scheduler job `oa-coder-host-image-daily` (us-central1) POSTs an
# inline build to the Cloud Build API once a day. The build is one step in the
# Cloud SDK image: a sparse clone of scripts/cloud at origin/main, then
# scripts/cloud/build-coder-host-image.sh --apply. No source repository
# connection or trigger is involved, so nothing else has to be kept in step.
#
# The build runs as the automation service account (it already holds
# compute.admin and iam.serviceAccountUser, which the bake needs) and writes
# its log to gs://openagentsgemini-oa-mvp-cloud-build-logs. A bake that fails
# leaves the family's previous images in place.
set -euo pipefail

PROJECT="${OA_PROJECT:-openagentsgemini}"
REGION="us-central1"
JOB="oa-coder-host-image-daily"
SCHEDULE="${OA_SCHEDULE:-0 7 * * *}"
RUNNER_SA="oa-mvp-automation@${PROJECT}.iam.gserviceaccount.com"
LOGS_BUCKET="gs://openagentsgemini-oa-mvp-cloud-build-logs"

mode="print"
case "${1:-}" in
  --apply) mode="apply" ;;
  --run-now) mode="run" ;;
  ""|--print) ;;
  *) echo "unknown argument: $1" >&2; exit 2 ;;
esac

step='set -euo pipefail
command -v jq >/dev/null || { apt-get update -qq >/dev/null && apt-get install -y -qq jq >/dev/null; }
git clone -q --depth 1 --filter=blob:none --sparse https://github.com/OpenAgentsInc/openagents.git oa
git -C oa sparse-checkout set scripts/cloud
oa/scripts/cloud/build-coder-host-image.sh --apply'

body="$(jq -n --arg sa "projects/$PROJECT/serviceAccounts/$RUNNER_SA" --arg logs "$LOGS_BUCKET" --arg step "$step" '{
  serviceAccount: $sa,
  logsBucket: $logs,
  options: {logging: "GCS_ONLY", machineType: "E2_MEDIUM"},
  timeout: "14400s",
  tags: ["oa-coder-host-image"],
  steps: [{name: "gcr.io/google.com/cloudsdktool/cloud-sdk:stable", entrypoint: "bash", args: ["-c", $step]}]
}')"
uri="https://cloudbuild.googleapis.com/v1/projects/$PROJECT/locations/$REGION/builds"

case "$mode" in
  print)
    echo "POST $uri"
    echo "$body"
    ;;
  apply)
    action=create
    if gcloud scheduler jobs describe "$JOB" --location "$REGION" --project "$PROJECT" >/dev/null 2>&1; then
      action=update
    fi
    gcloud scheduler jobs "$action" http "$JOB" \
      --location "$REGION" --project "$PROJECT" \
      --schedule "$SCHEDULE" --time-zone "Etc/UTC" \
      --uri "$uri" --http-method POST \
      --headers "Content-Type=application/json" \
      --message-body "$body" \
      --oauth-service-account-email "$RUNNER_SA" \
      --oauth-token-scope "https://www.googleapis.com/auth/cloud-platform" \
      --attempt-deadline 60s \
      --description "Daily oa-coder-host GCE image bake (docs/deployment/coder-host-image.md)"
    ;;
  run)
    gcloud scheduler jobs run "$JOB" --location "$REGION" --project "$PROJECT"
    ;;
esac
