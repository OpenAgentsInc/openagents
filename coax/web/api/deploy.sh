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
#
# Inference credit (inference.c, NIP-X402 over Spark): OA_SPARK_URL is oa-spark's URL when that
# service exists; the API's runtime account may invoke it (coax/web/api/spark/deploy.sh).
#
# Publishing from a drop (rc-publish, drops.c): Secret Manager oa-project-kek-staging and
# oa-project-kek-production (64 hex digits each, made once, never printed) seal each project's
# key; while one is missing that environment's PUBLISH only freezes the draft.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
commit="$(git -C "$repo" rev-parse --verify HEAD)"
[[ -z "$(git -C "$repo" status --porcelain --untracked-files=normal -- coax/web/api)" ]] || { echo 'api: commit API changes before deployment' >&2; exit 1; }
work="$(mktemp -d)"
cleanup() { git -C "$repo" worktree remove --force "$work/source" >/dev/null 2>&1 || true; rm -rf "$work"; }
trap cleanup EXIT
git -C "$repo" worktree add --detach "$work/source" "$commit" >/dev/null
here="$work/source/coax/web/api"
# shellcheck source=deploy-command.sh
source "$here/deploy-command.sh"
# shellcheck source=deploy-rollout.sh
source "$here/deploy-rollout.sh"
case "${1:-}" in '') prod=true ;; --staging) prod=false ;; *) echo 'usage: deploy.sh [--staging]' >&2; exit 1 ;; esac
project=openagentsgemini region=us-central1
instance="$project:$region:openagents-production-pg"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
"$here/test.sh"

stage="$work/context"
mkdir -p "$stage/src"
cp "$here"/*.c "$here"/*.h "$stage/src/"; cp -R "$here/migrations" "$here/Dockerfile" "$stage/"
tag="$region-docker.pkg.dev/$project/cloud-run-source-deploy/oa-api:${commit:0:12}-$(date +%Y%m%d-%H%M%S)"
# Capture the authenticated build result, not the mutable tag's later value.
gcloud builds submit "$stage" --project "$project" --region "$region" --tag "$tag" --format=json --suppress-logs --quiet \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET > "$work/build.json" 2> "$work/build.log" || { tail -30 "$work/build.log"; exit 1; }
image="$(python3 "$here/deploy_verify.py" build-image "$work/build.json" "$tag")"
admins="$(grep -v '^#' "$here/admins.conf" | grep -E '^[0-9a-f]{64}$' | paste -sd, - || true)"
runners="$(grep -v '^#' "$here/runners.conf" 2>/dev/null | grep -E '^[0-9a-f]{64}$' | paste -sd, - || true)"
# Our team (team.conf): kind:value,... ; its visits are our own testing, not usage.
team="$(grep -v '^#' "$here/team.conf" 2>/dev/null | awk 'NF == 2 && ($1 == "player" || $1 == "email_sha256") { print $1 ":" $2 }' | paste -sd, - || true)"

deploy() {  # service secret host agent-projects-secret runner-service drops-folder
  local secrets="OA_API_DATABASE=$2:latest" env="^@^OA_ADMIN_PUBKEYS=$admins@OA_AGENT_RUNNER_PUBKEYS=$runners@OA_TEAM=$team@OA_API_ORIGIN=https://$3@OA_DROPS_BUCKET=openagentsgemini-project-drops@OA_DROPS_PREFIX=$6" wake
  if gcloud secrets describe "$4" --project "$project" >/dev/null 2>&1; then secrets+=",OA_AGENT_PROJECTS=$4:latest"; fi
  # Publishing a project from a drop (drops.c, rc-publish): the key that seals project keys, the relay,
  # the site's bucket; staging's projects go on the relay as private, so no list shows them.
  local kek="oa-project-kek-$6" site="$project-openagents-web" vis=public
  [[ "$6" == staging ]] && site="$project-openagents-web-staging" vis=private
  if gcloud secrets describe "$kek" --project "$project" >/dev/null 2>&1; then
    secrets+=",OA_PROJECT_KEK=$kek:latest"
    env+="@OA_PUBLISH_RELAY=wss://relay.openagents.com@OA_SITE_BUCKET=$site@OA_PUBLISH_VISIBILITY=$vis"
  fi
  # Inference credit (inference.c): our Spark wallet, oa-spark (coax/web/api/spark/deploy.sh), when deployed.
  local spark
  spark="$(gcloud run services describe oa-spark --project "$project" --region "$region" --format 'value(status.url)' 2>/dev/null || true)"
  [[ -n "$spark" ]] && env+="@OA_SPARK_URL=$spark"
  wake="$(gcloud run services describe "$5" --project "$project" --region "$region" --format 'value(status.url)' 2>/dev/null || true)"
  [[ -n "$wake" ]] && env+="@OA_AGENT_WAKE_URL=$wake/wake"
  local release
  release="$(python3 -c 'import uuid; print(uuid.uuid4())')"
  env+="@OA_BUILD_SHA=$commit@OA_RELEASE_ID=$release"
  mkdir -p "$work/$1"
  api_verified_rollout "$1" "$3" "$image" "$commit" "$release" "$work/$1" \
    --allow-unauthenticated --ingress internal-and-cloud-load-balancing \
    --port 8080 --cpu 1 --memory 256Mi --min-instances 0 --max-instances 4 --concurrency 40 \
    --add-cloudsql-instances "$instance" --set-secrets "$secrets" --set-env-vars "$env"
}
deploy oa-api-staging oa-analytics-staging-database-url staging.openagents.com oa-agent-projects-staging oa-agent-staging staging
$prod && deploy oa-api oa-analytics-database-url openagents.com oa-agent-projects oa-agent production
echo "api: deployed $image"
