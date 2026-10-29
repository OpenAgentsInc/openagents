#!/usr/bin/env bash
# Deploys the project agent's runner (runner.c + the turn.coax engine) to
# Cloud Run: oa-agent-staging, then oa-agent, from one image. Scale to zero,
# at most one instance, CPU kept while a turn runs after the wake's answer,
# reachable only with an identity token (oa-api's service account invokes it).
#
#   coax/web/agent/deploy.sh            # build, staging, production
#   coax/web/agent/deploy.sh --staging  # stop after staging
#
# What projects it serves is server config, never in this repository: Secret
# Manager oa-agent-repos-staging and oa-agent-repos hold "slug=git-url=key-
# secret" entries (';' between them); each key secret is mounted as a file
# and the entry becomes OA_AGENT_REPOS. Staging runs Claude Haiku 5.5,
# production Claude Sonnet 5.5 (OA_AGENT_MODEL overrides both). After this,
# coax/web/api/deploy.sh sets oa-api's OA_AGENT_WAKE_URL to the runner.
#
# /chat (docs/chat.md) runs on the same runner: OA_CHAT_SEAL_KEY seals saved
# credentials (Secret Manager oa-chat-credential-key-staging and
# oa-chat-credential-key, 32 random bytes as hex, made here once), and each
# conversation's CLI session file is kept in a Cloud Storage bucket mounted
# at /sessions (oa-chat-sessions-staging, oa-chat-sessions; gen2).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
prod=true; [[ "${1:-}" == --staging ]] && prod=false
project=openagentsgemini region=us-central1
sa="oa-agent-runner@$project.iam.gserviceaccount.com"
invoker="157437760789-compute@developer.gserviceaccount.com"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"

stage="$(mktemp -d)"; trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/src"
"$repo/coax/bin/coax" c "$here/turn.coax" > "$stage/src/turn.c"
"$repo/coax/bin/coax" c "$here/chat.coax" > "$stage/src/chat.c"
cp "$here/runner.c" "$here/chat-runner.c" "$here/engine-io.c" "$repo/coax/platform/process-native.c" "$repo/coax/platform/process-native.h" "$stage/src/"
cp "$here/Dockerfile" "$here/known_hosts" "$stage/"
image="$region-docker.pkg.dev/$project/cloud-run-source-deploy/oa-agent:$(git -C "$here" rev-parse --short HEAD)-$(date +%Y%m%d-%H%M%S)"
gcloud builds submit "$stage" --project "$project" --region "$region" --tag "$image" --quiet \
  --service-account "projects/$project/serviceAccounts/oa-mvp-automation@$project.iam.gserviceaccount.com" \
  --default-buckets-behavior REGIONAL_USER_OWNED_BUCKET > "$stage/build.log" 2>&1 || { tail -30 "$stage/build.log"; exit 1; }

# /chat's sealing key and session bucket, made once each.
chat_store() {  # suffix ("" or -staging)
  local key="oa-chat-credential-key$1" bucket="$project-oa-chat-sessions$1"
  if ! gcloud secrets describe "$key" --project "$project" >/dev/null 2>&1; then
    gcloud secrets create "$key" --project "$project" --replication-policy automatic --quiet >/dev/null
    openssl rand -hex 32 | tr -d '\n' | gcloud secrets versions add "$key" --project "$project" --data-file - --quiet >/dev/null
  fi
  gcloud secrets add-iam-policy-binding "$key" --project "$project" --member "serviceAccount:$sa" --role roles/secretmanager.secretAccessor --quiet >/dev/null
  gcloud storage buckets describe "gs://$bucket" --project "$project" >/dev/null 2>&1 ||
    gcloud storage buckets create "gs://$bucket" --project "$project" --location "$region" --uniform-bucket-level-access --public-access-prevention --quiet >/dev/null
  gcloud storage buckets add-iam-policy-binding "gs://$bucket" --member "serviceAccount:$sa" --role roles/storage.objectAdmin --quiet >/dev/null
}

deploy() {  # service api-origin repos-secret model chat-suffix
  chat_store "$5"
  local secrets="OA_RUNNER_KEY=oa-agent-runner-key:latest,ANTHROPIC_API_KEY=openagents-anthropic-api-key:latest,OA_CHAT_SEAL_KEY=oa-chat-credential-key$5:latest" repos="" n=0 entry slug url key
  if gcloud secrets describe "$3" --project "$project" >/dev/null 2>&1; then
    IFS=';' read -ra entries <<< "$(gcloud secrets versions access latest --secret "$3" --project "$project")"
    for entry in "${entries[@]}"; do
      [[ -z "$entry" ]] && continue
      slug="${entry%%=*}"; key="${entry##*=}"; url="${entry#*=}"; url="${url%=*}"
      n=$((n + 1))
      secrets+=",/secrets/k$n/key=$key:latest"
      repos+="${repos:+;}$slug=$url=/secrets/k$n/key"
    done
  fi
  gcloud run deploy "$1" --image "$image" --project "$project" --region "$region" \
    --no-allow-unauthenticated --service-account "$sa" --no-cpu-throttling \
    --port 8080 --cpu 1 --memory 2Gi --min-instances 0 --max-instances 1 --concurrency 20 --timeout 900 \
    --execution-environment gen2 --clear-volumes --clear-volume-mounts \
    --add-volume "name=sessions,type=cloud-storage,bucket=$project-oa-chat-sessions$5,mount-options=uid=1000;gid=1000" \
    --add-volume-mount volume=sessions,mount-path=/sessions \
    --set-secrets "$secrets" \
    --set-env-vars "^|^OA_API_ORIGIN=$2|OA_AGENT_MODEL=$4|OA_AGENT_REPOS=$repos|OA_CHAT_SESSIONS=/sessions" --quiet 2>&1 | grep -E "ERROR|revision" || true
  gcloud run services add-iam-policy-binding "$1" --project "$project" --region "$region" \
    --member "serviceAccount:$invoker" --role roles/run.invoker --quiet >/dev/null
  echo "  ok    $1 ($n repositories)"
}
deploy oa-agent-staging https://staging.openagents.com oa-agent-repos-staging "${OA_AGENT_MODEL:-claude-haiku-5-5}" -staging
$prod && deploy oa-agent https://openagents.com oa-agent-repos "${OA_AGENT_MODEL:-claude-sonnet-5-5}" ""
echo "agent: deployed $image"
