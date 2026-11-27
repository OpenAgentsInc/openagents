#!/usr/bin/env bash
# Firm reason: 3 (Cloud Run / Cloud Build deployment wrapper for first-party GCP oa-boat service, #14128).
# Deploys oa-boat to Cloud Run (staging or production) with VPC egress and oa-boat-api-key secret.
# Requires owner approval for live GCE sandbox VM billing (see NEEDS_OWNER.md).
#
#   coax/web/boat/deploy.sh --staging
#   coax/web/boat/deploy.sh --production
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(git -C "$here" rev-parse --show-toplevel)"
env_name="staging"
case "${1:-}" in
  --staging) env_name="staging" ;;
  --production) env_name="production" ;;
  *) echo "usage: deploy.sh --staging | --production" >&2; exit 2 ;;
esac

project="${OA_GCP_PROJECT:-openagentsgemini}"
region="${OA_GCP_REGION:-us-central1}"
zone="${OA_BOAT_GCE_ZONE:-us-central1-a}"
service="oa-boat-$env_name"
[[ "$env_name" != production ]] || service="oa-boat"

commit="$(git -C "$repo" rev-parse --verify HEAD)"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

(cd "$repo" && coax/build.sh) >/dev/null
mkdir -p "$work/src" "$work/gen"
cp "$here/oa-boat.c" "$work/src/"
cp "$here/Dockerfile" "$work/"
"$repo/coax/bin/coax" c "$here/boat-service-wire.coax" --library --header "$work/gen/boat-service-wire.generated.h" > "$work/gen/boat-service-wire.generated.c"

tag="$region-docker.pkg.dev/$project/cloud-run-source-deploy/oa-boat:${commit:0:12}-$(date +%Y%m%d-%H%M%S)"
gcloud builds submit "$work" --project "$project" --region "$region" --tag "$tag" --quiet \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET

gcloud run deploy "$service" \
  --project "$project" --region "$region" --image "$tag" \
  --no-allow-unauthenticated --port 8080 --cpu 1 --memory 512Mi \
  --min-instances 0 --max-instances 4 --timeout 360 \
  --set-secrets "OA_BOAT_API_KEY=oa-boat-api-key:latest" \
  --set-env-vars "OA_BOAT_BACKEND=gce,OA_BOAT_GCE_PROJECT=$project,OA_BOAT_GCE_ZONE=$zone,OA_BOAT_MAX_ACTIVE=8" \
  --quiet
echo "oa-boat: deployed $service ($tag)"
