#!/usr/bin/env bash
# Deploys the voice host to Cloud Run as oa-voice: the Coax engine becomes C
# here (as build.sh does), the image builds on Cloud Build with Debian's gcc
# and libcurl, and the service answers WebSockets at its run.app address,
# which the web and phone clients use as wss://. Then it checks /health
# and runs one spoken turn through the browser's engine (coax/web/voice/smoke.ts).
#
#   verse/voice-host/deploy.sh
#
# One instance, so the meter (kept in memory) is one meter: 16 speakers at a
# time, each with the free tier and no budget (--max-speakers 16, Dockerfile).
# Google speech and Vertex AI are billed to openagentsgemini through the
# service's own account; no key is in the image.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
cd "$root"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
project=openagentsgemini region=us-central1 service=oa-voice
stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/src/gen" "$stage/src/platform"
# The engine as C, named as build.sh names it; #authority files run their checks inside `coax c`.
for src in verse/voice*.coax verse/voice-host/*.coax; do
  name="$(basename "$src" .coax)"
  base="$(printf '%s' "$name" | awk -F- '{for(i=1;i<=NF;i++) printf toupper(substr($i,1,1)) substr($i,2)}')"
  [[ "$base" == "Voice" ]] && base=VoiceEngine
  coax/bin/coax c "$src" --library --header "$stage/src/gen/$base.h" > "$stage/src/gen/$base.c"
done
cp verse/platform/voice.c verse/platform/voice.h "$stage/src/platform/"
cp "$here/host.c" "$here/ws.c" "$here/wav.c" "$here/speech.c" "$here/"*.h "$stage/src/" 2>/dev/null || true
cp "$here/Dockerfile" "$stage/"
image="$region-docker.pkg.dev/$project/cloud-run-source-deploy/$service:$(git rev-parse --short HEAD)-$(date +%Y%m%d-%H%M%S)"
gcloud builds submit "$stage" --project "$project" --region "$region" --tag "$image" --quiet \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET > "$stage/build.log" 2>&1 || { grep -E "error|undefined reference" "$stage/build.log" | head -30; exit 1; }
gcloud run deploy "$service" --image "$image" --project "$project" --region "$region" \
  --allow-unauthenticated --port 8080 --cpu 1 --memory 512Mi --cpu-boost \
  --min-instances 1 --max-instances 1 --concurrency 32 --timeout 3600 --session-affinity \
  --quiet 2>&1 | grep -E "ERROR|revision" || true
url="$(gcloud run services describe "$service" --project "$project" --region "$region" --format 'value(status.url)')"
[[ "$(curl -s -m 10 "$url/health")" == ok* ]] || { echo "voice: $url/health is not ok" >&2; exit 1; }
echo "voice: $url/health ok"
# One spoken turn through the browser's engine (voice.wasm), as the Haunted House talks to it.
coax/web/voice/build.sh "$stage/voice.wasm"
bun coax/web/voice/smoke.ts "$stage/voice.wasm" "wss://${url#https://}" "$here/fixtures/hello-foreman.wav" haunted-house
# The name the site and the app use (a Cloud Run domain mapping; Cloudflare CNAME to ghs.googlehosted.com).
bun coax/web/voice/smoke.ts "$stage/voice.wasm" wss://voice.openagents.com "$here/fixtures/hello-foreman.wav" haunted-house
echo "voice: deployed $image at wss://${url#https://}"
