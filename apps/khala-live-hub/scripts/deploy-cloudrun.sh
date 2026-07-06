#!/usr/bin/env bash
set -euo pipefail

# Deploy khala-live-hub to Cloud Run (CFG-5, #8520).
#
# Usage:
#   bash scripts/deploy-cloudrun.sh [staging|prod]     # default: staging
#
# Required before running:
#   gcloud auth login
#   gcloud config set project openagentsgemini
#
# Secrets (GCP Secret Manager):
#   khala-live-hub-token                — shared service bearer. Created with a
#                                         fresh random value on first deploy.
#   khala-live-hub-database-url-<env>   — DIRECT Postgres URL for window
#                                         rebuilds (khala_capture user against
#                                         khala_sync_<env> on khala-sync-pg).
#                                         Created from $KHALA_SYNC_DATABASE_URL
#                                         when absent; never printed.
#
# Deploy shape (deliberate — see src/server.ts module doc):
#   min=max=1 instance   — windows + sockets are in-memory per-scope state;
#                          sharding by scope hash is the documented extension
#                          point, not built at current scale.
#   session affinity     — WebSocket reconnects prefer the same instance.
#   timeout 3600         — Cloud Run's max; long live tails ride one request.
#   no-cpu-throttling    — the keepalive ping timer runs between requests.

ENVIRONMENT="${1:-staging}"
if [[ "$ENVIRONMENT" != "staging" && "$ENVIRONMENT" != "prod" ]]; then
  echo "usage: $0 [staging|prod]" >&2
  exit 1
fi

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REGION="${KHALA_LIVE_HUB_REGION:-us-central1}"
if [[ "$ENVIRONMENT" == "prod" ]]; then
  SERVICE="${KHALA_LIVE_HUB_SERVICE:-khala-live-hub}"
else
  SERVICE="${KHALA_LIVE_HUB_SERVICE:-khala-live-hub-staging}"
fi
TOKEN_SECRET="khala-live-hub-token"
DB_SECRET="khala-live-hub-database-url-${ENVIRONMENT}"

cd "$APP_DIR"

echo "==> Building Cloud Run server bundle"
bun run build:cloudrun

if ! gcloud secrets describe "$TOKEN_SECRET" >/dev/null 2>&1; then
  echo "==> Creating Secret Manager secret $TOKEN_SECRET (random value)"
  openssl rand -hex 32 | tr -d '\n' | gcloud secrets create "$TOKEN_SECRET" \
    --replication-policy=automatic \
    --data-file=-
else
  echo "==> Secret $TOKEN_SECRET already exists (leaving versions untouched)"
fi

if ! gcloud secrets describe "$DB_SECRET" >/dev/null 2>&1; then
  if [[ -z "${KHALA_SYNC_DATABASE_URL:-}" ]]; then
    echo "==> Secret $DB_SECRET is missing and \$KHALA_SYNC_DATABASE_URL is unset." >&2
    echo "    Create it first (direct khala_capture URL for khala_sync_${ENVIRONMENT}):" >&2
    echo "      printf '%s' \"\$URL\" | gcloud secrets create $DB_SECRET --replication-policy=automatic --data-file=-" >&2
    exit 1
  fi
  echo "==> Creating Secret Manager secret $DB_SECRET from \$KHALA_SYNC_DATABASE_URL"
  printf '%s' "$KHALA_SYNC_DATABASE_URL" | gcloud secrets create "$DB_SECRET" \
    --replication-policy=automatic \
    --data-file=-
else
  echo "==> Secret $DB_SECRET already exists (leaving versions untouched)"
fi

echo "==> Deploying $SERVICE to Cloud Run ($REGION, env $ENVIRONMENT)"
gcloud run deploy "$SERVICE" \
  --source . \
  --region "$REGION" \
  --allow-unauthenticated \
  --port 8080 \
  --timeout 3600 \
  --session-affinity \
  --min-instances 1 \
  --max-instances 1 \
  --concurrency 1000 \
  --no-cpu-throttling \
  --memory 512Mi \
  --set-secrets "KHALA_LIVE_HUB_TOKEN=${TOKEN_SECRET}:latest,KHALA_SYNC_DATABASE_URL=${DB_SECRET}:latest"

URL="$(gcloud run services describe "$SERVICE" --region "$REGION" --format='value(status.url)')"
echo "==> Done. Service URL: $URL"
echo "    Verify:"
echo "      curl -s $URL/health                                   # => {\"ok\":true,...}"
echo "      curl -s -o /dev/null -w '%{http_code}' $URL/log        # => 401 (fail closed)"
