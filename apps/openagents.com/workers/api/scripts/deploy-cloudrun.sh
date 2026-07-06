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
)

if [[ "$TARGET" == "production" ]]; then
  SET_SECRETS+=(
    "GITHUB_CLIENT_SECRET=openagents-github-client-secret:latest"
    # D1-over-HTTP bridge for not-yet-migrated CFG-4 domains (typed 503 when
    # the daily free-tier quota is exhausted — see #8524).
    "CLOUDFLARE_API_TOKEN=openagents-monolith-cf-d1-token:latest"
  )
fi

SECRET_FLAG="$(IFS=,; echo "${SET_SECRETS[*]}")"

EXTRA_ENV=""
if [[ "$TARGET" == "production" ]]; then
  EXTRA_ENV="CLOUDFLARE_ACCOUNT_ID=54fac8b750a29fdda9f2fa0f0afaed90,CLOUDFLARE_D1_DATABASE_ID=9644ea09-f682-4971-98de-e0c791cb67fb"
fi

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
  ${EXTRA_ENV:+--update-env-vars "$EXTRA_ENV"} \
  --set-secrets "$SECRET_FLAG"

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
