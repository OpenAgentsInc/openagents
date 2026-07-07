#!/usr/bin/env bash
set -euo pipefail

# Deploy the openagents.com monolith to Google Cloud Run (CFG-9, #8524).
#
# Usage:
#   scripts/deploy-cloudrun.sh staging      # openagents-monolith-staging
#   scripts/deploy-cloudrun.sh production   # openagents-monolith
#
# Build happens here on the deploy machine (aiur/oa-updates pattern):
#   1. bun run build:web (repo apps/web SPA assets)
#   2. bun build src/cloudrun/server.ts + preload.ts → dist-cloudrun/
#   3. render the non-secret env YAML from wrangler.jsonc vars
#   4. gcloud run deploy --source . (Dockerfile in this directory)
#
# Secrets ride --set-secrets from GCP Secret Manager (created out of band —
# never from tracked files; see the CFG-9 secret map on issue #8524):
#   openagents-monolith-database-url-<env>   direct Cloud SQL Postgres URL
#                                            (khala_app; kills Hyperdrive)
#   openagents-monolith-cron-token-<env>     bearer for POST /internal/cron
#   openagents-monolith-admin-token-<env>    OPENAGENTS_ADMIN_API_TOKEN
#   khala-live-hub-token                     shared LiveHub service bearer
#   openagents-gemini-api-key / openagents-openrouter-api-key /
#   openagents-fireworks-api-key / openagents-exa-api-key /
#   openagents-resend-api-key / openagents-vertex-sa-key
#   openagents-stripe-api-key-<test|live>
#   openagents-github-client-secret          (production only; NEEDS-OWNER
#                                            until re-supplied — see #8524)
#
# Cloud Scheduler: pass --with-scheduler to (re)create the per-minute
# /internal/cron job for the target env after deploy.

TARGET="${1:-}"
if [[ "$TARGET" != "staging" && "$TARGET" != "production" ]]; then
  echo "usage: $0 (staging|production) [--with-scheduler]" >&2
  exit 2
fi
WITH_SCHEDULER="${2:-}"

PROJECT="${OPENAGENTS_GCP_PROJECT:-openagentsgemini}"
REGION="${OPENAGENTS_GCP_REGION:-us-central1}"

if [[ "$TARGET" == "production" ]]; then
  SERVICE="openagents-monolith"
  ENV_SUFFIX="prod"
  STRIPE_SECRET="openagents-stripe-api-key-live"
else
  SERVICE="openagents-monolith-staging"
  ENV_SUFFIX="staging"
  STRIPE_SECRET="openagents-stripe-api-key-test"
fi

API_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$(cd "$API_DIR/../.." && pwd)"   # apps/openagents.com

cd "$APP_DIR"
echo "==> Building web assets (apps/web/dist)"
bun run build:web >/dev/null

cd "$API_DIR"
echo "==> Bundling server + preload (bun build --target=bun)"
rm -rf dist-cloudrun
bun build src/cloudrun/server.ts --target bun --outdir dist-cloudrun \
  --external cloudflare:workers --external '@cloudflare/playwright' >/dev/null
bun build src/cloudrun/preload.ts --target bun --outdir dist-cloudrun >/dev/null
cp -R "$APP_DIR/apps/web/dist" dist-cloudrun/web-dist

echo "==> Rendering env vars from wrangler.jsonc ($TARGET)"
bun scripts/cloudrun/render-env-yaml.ts "$TARGET"

SET_SECRETS=(
  "KHALA_SYNC_DATABASE_URL=openagents-monolith-database-url-${ENV_SUFFIX}:latest"
  "CLOUD_RUN_CRON_TOKEN=openagents-monolith-cron-token-${ENV_SUFFIX}:latest"
  "OPENAGENTS_ADMIN_API_TOKEN=openagents-monolith-admin-token-${ENV_SUFFIX}:latest"
  "KHALA_SYNC_LIVE_HUB_TOKEN=khala-live-hub-token:latest"
  "GEMINI_API_KEY=openagents-gemini-api-key:latest"
  "OPENROUTER_API_KEY=openagents-openrouter-api-key:latest"
  "FIREWORKS_API_KEY=openagents-fireworks-api-key:latest"
  "EXA_API_KEY=openagents-exa-api-key:latest"
  "RESEND_API_KEY=openagents-resend-api-key:latest"
  "VERTEX_SA_KEY=openagents-vertex-sa-key:latest"
  "STRIPE_API_KEY=${STRIPE_SECRET}:latest"
  # Cloud coding sessions control-plane bearer (oa-cloud-run-bridge).
  "OA_CLOUD_CONTROL_TOKEN=oa-cloud-run-bridge-control-token:latest"
  # CFG-8 GCS artifacts (bucket name is a committed wrangler var).
  "ARTIFACTS_GCS_HMAC_ACCESS_KEY_ID=oa-artifacts-gcs-hmac-access-key-id:latest"
  "ARTIFACTS_GCS_HMAC_SECRET=oa-artifacts-gcs-hmac-secret:latest"
  "AGENT_REGISTRATION_SECRET=openagents-agent-registration-secret:latest"
  "ARTANIS_AGENT_TOKEN=openagents-artanis-agent-token:latest"
  # CFG-14 (2026-07-07): khala_app password for the Cloud SQL Auth Connector
  # socket path (PGHOST/PGUSER are non-secret wrangler vars; the db name rides
  # the authority-less KHALA_SYNC_DATABASE_URL secret). khala_app is
  # instance-wide, so the same secret serves prod + staging. WITHOUT this the
  # deploy drops PGPASSWORD and the socket connection fails — and with public
  # ingress closed the DB is unreachable.
  "PGPASSWORD=openagents-monolith-pgpassword:latest"
)

if [[ "$TARGET" == "production" ]]; then
  SET_SECRETS+=(
    # CFG-15: service tokens the monolith presents to the Cloud Run MDK
    # money-path daemons (x-treasury-service-token / x-tips-buffer-service-token).
    # The matching run.app URLs are committed wrangler vars. Sidecar token is
    # omitted until the sidecar cutover lands.
    "MDK_TREASURY_SERVICE_TOKEN=mdk-treasury-service-token:latest"
    "MDK_TIPS_BUFFER_SERVICE_TOKEN=mdk-tips-buffer-service-token:latest"
    "GITHUB_CLIENT_SECRET=openagents-github-client-secret:latest"
    # SHC live dispatch (config validation requires the bearer when
    # SHC_DISPATCH_MODE=live).
    "SHC_CONTROL_API_BEARER_TOKEN=openagents-shc-control-api-bearer:latest"
    "SHC_RUNNER_CALLBACK_TOKEN=openagents-shc-runner-callback-token:latest"
    # D1-over-HTTP bridge for not-yet-migrated CFG-4 domains (typed 503 when
    # the daily free-tier quota is exhausted — see #8524).
    "CLOUDFLARE_API_TOKEN=openagents-monolith-cf-d1-token:latest"
    # Hydralisk GPT-OSS lanes (secondary; 120B base URL is CF-only — see the
    # #8524 NEEDS-OWNER catalogue).
    "HYDRALISK_BASE_URL=hydralisk-gptoss20b-base-url:latest"
    "HYDRALISK_BEARER_TOKEN=hydralisk-gptoss20b-bearer:latest"
    "HYDRALISK_GPT_OSS_120B_BEARER_TOKEN=hydralisk-gptoss120b-bearer:latest"
  )
  # Hydralisk GLM-5.2-REAP-504B fleet (the Khala primary backing): one
  # BASE_URL + BEARER_TOKEN pair per replica id from the committed
  # HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS wrangler var.
  GLM_REPLICAS=(
    g4-4g-b-20260625154532
    g4-4g-central1f-spot-20260625203000
    g4-4g-east1b-spot-20260625203000
    g4-4g-east1d-spot-20260625203000
    g4-4g-east5a-spot-20260625203000
    g4-4g-east5b-spot-20260625203000
    g4-4g-east5c-spot-20260625211500
    g4-4g-south1b-spot-20260625211500
    g4-4g-west1a-spot-20260625203000
    g4-8g-b-20260624214500
  )
  for replica in "${GLM_REPLICAS[@]}"; do
    suffix="$(echo "$replica" | tr '[:lower:]-' '[:upper:]_')"
    SET_SECRETS+=(
      "HYDRALISK_GLM_52_REAP_504B_${suffix}_BASE_URL=hydralisk-glm-${replica}-base-url:latest"
      "HYDRALISK_GLM_52_REAP_504B_${suffix}_BEARER_TOKEN=hydralisk-glm-${replica}-bearer:latest"
    )
  done
fi

SECRET_FLAG="$(IFS=,; echo "${SET_SECRETS[*]}")"

echo "==> Deploying $SERVICE to Cloud Run ($REGION)"
gcloud run deploy "$SERVICE" \
  --project "$PROJECT" \
  --source . \
  --region "$REGION" \
  --allow-unauthenticated \
  --port 8080 \
  --min-instances 1 \
  --max-instances 4 \
  --cpu 2 \
  --memory 2Gi \
  --timeout 3600 \
  --concurrency 80 \
  --env-vars-file "dist-cloudrun/env-${TARGET}.yaml" \
  --set-secrets "$SECRET_FLAG" \
  --add-cloudsql-instances "openagentsgemini:us-central1:khala-sync-pg"

SERVICE_URL="$(gcloud run services describe "$SERVICE" --project "$PROJECT" --region "$REGION" --format='value(status.url)')"
echo "==> Deployed: $SERVICE_URL"

if [[ "$WITH_SCHEDULER" == "--with-scheduler" ]]; then
  echo "==> Ensuring per-minute Cloud Scheduler cron ($SERVICE-cron)"
  CRON_TOKEN="$(gcloud secrets versions access latest --secret "openagents-monolith-cron-token-${ENV_SUFFIX}" --project "$PROJECT")"
  gcloud scheduler jobs delete "$SERVICE-cron" --project "$PROJECT" --location "$REGION" --quiet 2>/dev/null || true
  gcloud scheduler jobs create http "$SERVICE-cron" \
    --project "$PROJECT" \
    --location "$REGION" \
    --schedule "* * * * *" \
    --uri "${SERVICE_URL}/internal/cron" \
    --http-method POST \
    --headers "Authorization=Bearer ${CRON_TOKEN}" \
    --attempt-deadline 300s
fi

echo "==> Smoke: /internal/healthz"
curl -fsS "${SERVICE_URL}/internal/healthz"
echo
echo "==> Done."
