#!/usr/bin/env bash
# coax deploy, for now: builds an app in release, stages it with the static
# server, and deploys it to Cloud Run. Usage: deploy.sh APP_DIR SERVICE
# The first deploy of a service also needs its domain mapping (see web.md).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
app="$(cd "$1" && pwd)"; service="$2"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
COAXIAL_MODE=release "$app/build.sh"
stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/server" "$stage/coaxial" "$stage/app/build"
cp "$here/serve.c" "$here/../../metareal/voice-host/ws.c" "$here/../../metareal/voice-host/ws.h" "$stage/server/"
sed -i '' 's#"../../metareal/voice-host/ws.h"#"ws.h"#' "$stage/server/serve.c"
cp "$here/host.js" "$stage/coaxial/"
cp "$here/Dockerfile" "$stage/"
# the page's files, not its sources or build intermediates
for f in "$app"/*.html "$app"/*.md "$app"/*.svg "$app"/*.wgsl; do
  [ -e "$f" ] && [ "$(basename "$f")" != README.md ] && cp "$f" "$stage/app/"
done
cp "$app"/build/*.wasm "$stage/app/build/"
image="us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/$service:$(date +%Y%m%d-%H%M%S)"
gcloud builds submit "$stage" --project openagentsgemini --region us-central1 --tag "$image" --quiet \
  --service-account projects/openagentsgemini/serviceAccounts/oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET
gcloud run deploy "$service" --image "$image" --project openagentsgemini --region us-central1 \
  --allow-unauthenticated --port 8080 --cpu 1 --memory 128Mi --min-instances 0 --max-instances 3 \
  --concurrency 200 --quiet "${@:3}"
