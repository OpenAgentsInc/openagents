#!/usr/bin/env bash
# Deploys the API service (api.c) to Cloud Run: staging first, then
# production with the same image.
#
#   coax/web/api/deploy.sh            # build, staging, smoke, production, smoke
#   coax/web/api/deploy.sh --staging  # stop after staging
#
# Services: oa-api-staging (database oa_analytics_staging) and oa-api
# (oa_analytics), both on the Cloud SQL instance openagents-production-pg,
# reachable only through the openagents.com load balancer (path rule /api/
# on hosts staging.openagents.com and openagents.com; coax/web/api/lb.sh).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
prod=true; [[ "${1:-}" == --staging ]] && prod=false
project=openagentsgemini region=us-central1
instance="$project:$region:openagents-production-pg"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
"$here/test.sh"

stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/src"
cp "$here"/*.c "$here"/*.h "$stage/src/"; cp -R "$here/migrations" "$here/Dockerfile" "$stage/"
image="$region-docker.pkg.dev/$project/cloud-run-source-deploy/oa-api:$(git -C "$here" rev-parse --short HEAD)-$(date +%Y%m%d-%H%M%S)"
gcloud builds submit "$stage" --project "$project" --region "$region" --tag "$image" --quiet \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET > "$stage/build.log" 2>&1 || { tail -30 "$stage/build.log"; exit 1; }
admins="$(grep -v '^#' "$here/admins.conf" | grep -E '^[0-9a-f]{64}$' | paste -sd, - || true)"

deploy() {  # service secret host
  gcloud run deploy "$1" --image "$image" --project "$project" --region "$region" \
    --allow-unauthenticated --ingress internal-and-cloud-load-balancing \
    --port 8080 --cpu 1 --memory 256Mi --min-instances 0 --max-instances 4 --concurrency 40 \
    --add-cloudsql-instances "$instance" --set-secrets "OA_API_DATABASE=$2:latest" \
    --set-env-vars "^@^OA_ADMIN_PUBKEYS=$admins@OA_API_ORIGIN=https://$3" --quiet 2>&1 | grep -E "ERROR|revision" || true
  for _ in $(seq 20); do
    body="$(curl -s "https://$3/api/playtest/health" || true)"
    [[ "$body" == *'"ok":true'* ]] && { echo "  ok    https://$3/api/playtest/health $body"; return 0; }
    sleep 3
  done
  echo "  FAIL  https://$3/api/playtest/health: $body"; return 1
}
deploy oa-api-staging oa-analytics-staging-database-url staging.openagents.com
$prod && deploy oa-api oa-analytics-database-url openagents.com
echo "api: deployed $image"
