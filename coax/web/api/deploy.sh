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
#
# The project agent (docs/project-agent.md): the approved projects are server
# config, never in this repository: Secret Manager oa-agent-projects-staging
# and oa-agent-projects (comma-separated slugs) become OA_AGENT_PROJECTS when
# they exist, and the runner services oa-agent-staging and oa-agent, when
# deployed (coax/web/agent/deploy.sh), become OA_AGENT_WAKE_URL.
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

deploy() {  # service secret host agent-projects-secret runner-service
  local secrets="OA_API_DATABASE=$2:latest" env="^@^OA_ADMIN_PUBKEYS=$admins@OA_API_ORIGIN=https://$3" wake
  if gcloud secrets describe "$4" --project "$project" >/dev/null 2>&1; then secrets+=",OA_AGENT_PROJECTS=$4:latest"; fi
  wake="$(gcloud run services describe "$5" --project "$project" --region "$region" --format 'value(status.url)' 2>/dev/null || true)"
  [[ -n "$wake" ]] && env+="@OA_AGENT_WAKE_URL=$wake/wake"
  gcloud run deploy "$1" --image "$image" --project "$project" --region "$region" \
    --allow-unauthenticated --ingress internal-and-cloud-load-balancing \
    --port 8080 --cpu 1 --memory 256Mi --min-instances 0 --max-instances 4 --concurrency 40 \
    --add-cloudsql-instances "$instance" --set-secrets "$secrets" \
    --set-env-vars "$env" --quiet 2>&1 | grep -E "ERROR|revision" || true
  for _ in $(seq 20); do
    body="$(curl -s "https://$3/api/playtest/health" || true)"
    [[ "$body" == *'"ok":true'* ]] && { echo "  ok    https://$3/api/playtest/health $body"; return 0; }
    sleep 3
  done
  echo "  FAIL  https://$3/api/playtest/health: $body"; return 1
}
deploy oa-api-staging oa-analytics-staging-database-url staging.openagents.com oa-agent-projects-staging oa-agent-staging
$prod && deploy oa-api oa-analytics-database-url openagents.com oa-agent-projects oa-agent
echo "api: deployed $image"
