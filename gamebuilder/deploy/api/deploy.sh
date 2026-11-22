#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper around gcloud; holds no logic).
# The hosted levels API on Cloud Run (docs/programmatic-access.md, C6 and C7): the container in
# this folder, each account's level files in a storage bucket mounted at /data, reachable only
# through gamebuilder.org's load balancer.
#
# It makes cloud resources that cost money: run it only with the owner's go-ahead
# (NEEDS_OWNER.md). Nothing in the repository's checks calls it.
#
#   deploy/api/deploy.sh          one time and every release: the bucket, the image, the service,
#                                 and the load balancer's backend for it. Safe to run again.
#   deploy/api/deploy.sh status   the service's URL and its latest revision
#
# Then `scripts/web-deploy.sh domain` sends the API's paths (/api/rows/*, /api/levels/*, /api/play/*,
# /g/*, /api/capabilities, /api/openapi.json, /api/count, /mcp) to it: it adds them only once the backend exists.
# /g/CODE is a published game's link (docs/publishing.md, "What is built"): the service fetches the
# site's own /play page from GB_ORIGIN to make the link's page. GAMES_DOMAIN (unset: GB_ORIGIN) is
# where the links it says begin, the day games move to a domain of their own.
#
# One instance at most: a level's revision is checked and its file written in one process, so two
# instances could each accept a write at the same revision.
set -euo pipefail
gb="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo="$(cd "$gb/.." && pwd)"
export GB_ROOT="$gb" GB_VENDOR="$repo" COAX="${COAX:-$repo/coax/bin/coax}"
project=openagentsgemini
region=us-central1
name=gamebuilder-api
bucket=openagentsgemini-gamebuilder-levels
image="$region-docker.pkg.dev/$project/gamebuilder/$name"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
g() { gcloud "$@" --project "$project" --quiet; }
has() { g "$@" > /dev/null 2>&1; }

if [[ "${1:-}" == status ]]; then
  g run services describe "$name" --region "$region" --format 'value(status.url,status.latestReadyRevisionName)'
  exit
fi

# What is deployed is a commit on main, never a working tree.
[[ -z "$(git -C "$repo" status --porcelain -- gamebuilder)" ]] || { echo "deploy/api: gamebuilder has uncommitted changes; deploy a commit" >&2; exit 1; }
commit="$(git -C "$repo" rev-parse HEAD)"

stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
# Only what the CLI's build loads (its compiler, libraries, gamebuilder and the engine), not the whole
# tree. coax/tests stays (a library loads from it), and scripts/coax-release.sh is one of the compiler's checked inputs.
git -C "$repo" archive "$commit" -- gamebuilder coax apps/openagents-cli components metaverse-engine metaverse/splice-bots psionic/lib \
  scripts/coax-release.sh | tar -x -C "$stage"
# the commit it is, for the version it reports where there is no git
printf '%s\n' "$commit" > "$stage/gamebuilder/.commit"
cp "$GB_ROOT/deploy/api/Dockerfile" "$stage/Dockerfile"

has storage buckets describe "gs://$bucket" || g storage buckets create "gs://$bucket" --location "$region" --uniform-bucket-level-access --public-access-prevention
has artifacts repositories describe gamebuilder --location "$region" || g artifacts repositories create gamebuilder --location "$region" --repository-format docker
# The org's policy wants a named build account: the automation account, as openagents' deploys use.
g builds submit "$stage" --region "$region" --tag "$image:$commit" --timeout 3600s --machine-type e2-highcpu-8 \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET

# The bucket is mounted at /data (Cloud Run volume, Cloud Storage FUSE). The service takes only
# the load balancer's traffic; its callers are signed in, and the server asks the login service
# at gamebuilder.org who each one is.
# Cloud agents (issue 7; docs/agents.md, "Built: a cloud agent"): the server signs its model calls with
# its own Nostr key, the secret gamebuilder-model-key (64 hex, its public key listed in openagents.com's
# OA_INFERENCE_PUBKEYS), mounted as a file. Without the secret the server runs no cloud agents and says
# so. Their steps run between requests, so the CPU is not throttled between them.
cloud=()
if has secrets describe gamebuilder-model-key; then
  cloud=(--set-secrets "/secrets/model/key=gamebuilder-model-key:latest" --no-cpu-throttling)
  envs="GB_ORIGIN=https://gamebuilder.org,GB_ACCOUNTS=/data,GB_MODEL_KEY_FILE=/secrets/model/key"
else
  envs="GB_ORIGIN=https://gamebuilder.org,GB_ACCOUNTS=/data"
fi
g run deploy "$name" --region "$region" --image "$image:$commit" \
  --execution-environment gen2 --max-instances 1 --min-instances 0 --cpu 1 --memory 512Mi \
  --ingress internal-and-cloud-load-balancing --allow-unauthenticated \
  --add-volume "name=levels,type=cloud-storage,bucket=$bucket" --add-volume-mount "volume=levels,mount-path=/data" \
  --set-env-vars "$envs" ${cloud[@]+"${cloud[@]}"} \
  --labels "commit=${commit:0:12}"

# The load balancer's way to it: a serverless network endpoint group and a backend service.
has compute network-endpoint-groups describe "$name" --region "$region" || \
  g compute network-endpoint-groups create "$name" --region "$region" --network-endpoint-type serverless --cloud-run-service "$name"
if ! has compute backend-services describe "$name" --global; then
  g compute backend-services create "$name" --global --load-balancing-scheme EXTERNAL_MANAGED
  g compute backend-services add-backend "$name" --global --network-endpoint-group "$name" --network-endpoint-group-region "$region"
fi
"$0" status
# The site now answers with this commit's command table, or the deploy didn't take: compare the
# live GET /api/capabilities with this build's (docs/programmatic-access.md, "Versions").
bin="$("$GB_ROOT/cli/build.sh" | tail -1)"
for try in 1 2 3 4 5 6; do
  if "$bin" --remote version; then exit 0; fi
  sleep 20
done
echo "deploy/api: gamebuilder.org still answers another command table than this commit's" >&2
exit 1
