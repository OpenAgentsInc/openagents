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

coax_bin="${COAX:-$repo/coax/bin/coax}"
if [[ ! -x "$coax_bin" ]]; then
  (cd "$repo" && coax/build.sh) >/dev/null
fi
mkdir -p "$work/src" "$work/gen"
cp "$here/oa-boat.c" "$work/src/"
cp "$here/Dockerfile" "$work/"
"$coax_bin" c "$here/boat-service-wire.coax" --library --header "$work/gen/boat-service-wire.generated.h" > "$work/gen/boat-service-wire.generated.c"

tag="$region-docker.pkg.dev/$project/cloud-run-source-deploy/oa-boat:${commit:0:12}-$(date +%Y%m%d-%H%M%S)"
gcloud builds submit "$work" --project "$project" --region "$region" --tag "$tag" --quiet \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET

# Least-privilege runtime SA (compute.instanceAdmin.v1 + compute.storageAdmin + secretAccessor on oa-boat-api-key & oa-boat-ssh-key).
# Direct VPC egress on default/default reaches internal 10.128.x.x GCE VMs over SSH without external IPs;
# application-layer constant-time Bearer auth (bs_c_auth_ok + OA_BOAT_API_KEY) protects all routes.
gcloud run deploy "$service" \
  --project "$project" --region "$region" --image "$tag" \
  --service-account "oa-boat@$project.iam.gserviceaccount.com" \
  --network default --subnet default --vpc-egress private-ranges-only \
  --no-cpu-throttling \
  --allow-unauthenticated --port 8080 --cpu 1 --memory 512Mi \
  --min-instances 1 --max-instances 1 --timeout 600 \
  --set-secrets "OA_BOAT_API_KEY=oa-boat-api-key:latest,/var/run/oa-boat/ssh_key=oa-boat-ssh-key:latest" \
  --set-env-vars "OA_BOAT_BACKEND=gce,OA_BOAT_GCE_PROJECT=$project,OA_BOAT_GCE_ZONE=$zone,OA_BOAT_GCE_IMAGE=${OA_BOAT_GCE_IMAGE:-projects/$project/global/images/family/oa-sandbox},OA_BOAT_MAX_ACTIVE=8,OA_BOAT_SSH_KEY_FILE=/var/run/oa-boat/ssh_key" \
  --quiet
echo "oa-boat: deployed $service ($tag)"
