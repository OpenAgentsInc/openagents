#!/usr/bin/env bash
# Build from main and deploy the lifecycle service with its account database.
# Usage: deploy/oa-computers/deploy.sh staging|production [COMMIT]
set -euo pipefail
sdk_environment=${1:?Choose staging or production.}
sdk_commit=${2:-$(git rev-parse HEAD)}
sdk_project=openagentsgemini
sdk_region=us-central1
sdk_repo=us-central1-docker.pkg.dev/$sdk_project/cloud-run-source-deploy/oa-computers
sdk_boat_base=https://oa-boat-157437760789.us-central1.run.app/api/v1
case "$sdk_environment" in
 staging)
  sdk_boat_base=${OA_COMPUTERS_BOAT_API_BASE:-$sdk_boat_base}
  sdk_service=oa-computers-staging
  sdk_database=openagents-staging-pg
  sdk_dsn=openagents-staging-pg-dsn
  sdk_token=oa-computers-staging-token
  sdk_runtime=oa-vertex-inference@$sdk_project.iam.gserviceaccount.com
  ;;
 production)
  sdk_service=oa-computers
  sdk_database=openagents-production-pg
  sdk_dsn=openagents-production-pg-dsn
  sdk_token=oa-computers-token
  sdk_runtime=157437760789-compute@developer.gserviceaccount.com
  ;;
 *) echo 'Choose staging or production.' >&2; exit 2 ;;
esac
[[ $sdk_commit =~ ^[0-9a-f]{40}$ ]] || { echo 'Use a full commit ID.' >&2; exit 2; }
git fetch -q origin main
git merge-base --is-ancestor "$sdk_commit" origin/main || { echo 'Push the commit to main before deploying.' >&2; exit 1; }
# Verify the previous connection before assigning rows written by the old
# single-driver service. A new default is not evidence of their deployment.
sdk_legacy_mapping() {
 gcloud run services describe "$sdk_service" --project "$sdk_project" --region "$sdk_region" --format=json |
 python3 -c '
import json,sys
service=json.load(sys.stdin)
env={e["name"]:e.get("value") for e in service["spec"]["template"]["spec"]["containers"][0].get("env",[])}
if (env.get("OA_COMPUTERS_REGISTRY_FILE") or
    env.get("OA_COMPUTERS_DRIVER","boat") != "boat" or
    env.get("BOAT_API_BASE","https://api.boat.dev/api/v1").rstrip("/") != sys.argv[1].rstrip("/") or
    env.get("OA_COMPUTERS_LEGACY_DEPLOYMENT", "oa-boat-primary") != "oa-boat-primary"):
    sys.exit("The running computer connection differs. Keep its deployment mapping before using this helper.")
print("oa-boat-primary")
' "$sdk_boat_base"
}
sdk_legacy=$(sdk_legacy_mapping)
if [[ $sdk_environment == staging ]]; then
 if ! gcloud secrets describe "$sdk_token" --project "$sdk_project" >/dev/null 2>&1; then
  python3 -c 'import secrets; print(secrets.token_urlsafe(48), end="")' |
   gcloud secrets create "$sdk_token" --project "$sdk_project" --replication-policy=automatic --data-file=- >/dev/null
 fi
 gcloud secrets add-iam-policy-binding "$sdk_token" --project "$sdk_project" \
  --member="serviceAccount:$sdk_runtime" --role=roles/secretmanager.secretAccessor >/dev/null
fi

sdk_digest=$(gcloud artifacts docker images describe "$sdk_repo:$sdk_commit" --project "$sdk_project" \
 --format='value(image_summary.digest)' 2>/dev/null || true)
if [[ -z $sdk_digest ]]; then
 git merge-base --is-ancestor origin/main "$sdk_commit" || { echo 'Rebase onto current main before building.' >&2; exit 1; }
 gcloud builds submit --no-source --config deploy/oa-computers/cloudbuild.yaml \
  --substitutions "_COMMIT=$sdk_commit" --project "$sdk_project" --region "$sdk_region" \
  --service-account "projects/$sdk_project/serviceAccounts/oa-mvp-automation@$sdk_project.iam.gserviceaccount.com" \
  --gcs-log-dir "gs://openagentsgemini-oa-mvp-cloud-build-logs/$sdk_service"
 sdk_digest=$(gcloud artifacts docker images describe "$sdk_repo:$sdk_commit" --project "$sdk_project" \
  --format='value(image_summary.digest)')
fi
[[ $sdk_digest =~ ^sha256:[0-9a-f]{64}$ ]] || { echo 'The built image digest is unavailable.' >&2; exit 1; }

# A newer deployment must not be replaced by an older checkout after the build.
sdk_previous=$(gcloud run services describe "$sdk_service" --project "$sdk_project" --region "$sdk_region" \
 --format='value(spec.template.metadata.labels.openagents-commit)' 2>/dev/null || true)
if [[ -z $sdk_previous ]]; then
 sdk_image=$(gcloud run services describe "$sdk_service" --project "$sdk_project" --region "$sdk_region" \
  --format='value(spec.template.spec.containers[0].image)' 2>/dev/null || true)
 if [[ $sdk_image =~ :([0-9a-f]{40})$ ]]; then sdk_previous=${BASH_REMATCH[1]}; fi
fi
if [[ -n $sdk_previous ]]; then
 git fetch -q origin main
 git merge-base --is-ancestor "$sdk_previous" "$sdk_commit" || {
  echo 'A newer service build is running. Rebase and rebuild before deploying.' >&2; exit 1;
 }
fi

sdk_legacy=$(sdk_legacy_mapping)
gcloud run deploy "$sdk_service" --project "$sdk_project" --region "$sdk_region" \
 --image "$sdk_repo@$sdk_digest" --service-account "$sdk_runtime" \
 --add-cloudsql-instances "$sdk_project:$sdk_region:$sdk_database" \
 --update-secrets "OA_COMPUTERS_DATABASE_URL=$sdk_dsn:latest,OA_COMPUTERS_TOKEN=$sdk_token:latest,BOAT_API_KEY=oa-boat-api-key:latest" \
 --update-env-vars "OA_COMPUTERS_DRIVER=boat,BOAT_API_BASE=$sdk_boat_base,OA_COMPUTERS_LEGACY_DEPLOYMENT=$sdk_legacy" \
 --labels "openagents-commit=$sdk_commit" --min-instances 1 --max-instances 1 \
 --no-cpu-throttling --cpu 1 --memory 512Mi --allow-unauthenticated --quiet
sdk_running=$(gcloud run services describe "$sdk_service" --project "$sdk_project" --region "$sdk_region" \
 --format='value(spec.template.metadata.labels.openagents-commit)')
[[ $sdk_running == "$sdk_commit" ]] || { echo 'The running service commit does not match.' >&2; exit 1; }
sdk_running_image=$(gcloud run services describe "$sdk_service" --project "$sdk_project" --region "$sdk_region" \
 --format='value(spec.template.spec.containers[0].image)')
[[ $sdk_running_image == "$sdk_repo@$sdk_digest" ]] || { echo 'The running service image does not match.' >&2; exit 1; }
printf '%s is running %s (%s)\n' "$sdk_service" "$sdk_running" "$sdk_digest"
