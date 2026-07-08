#!/usr/bin/env bash
set -euo pipefail

# Deploy khala-capture (the Khala Sync capture daemon) to Cloud Run (#8554).
#
# Replaces the launchd/Mac capture path (frozen by CFG-14, which closed the
# Cloud SQL public ingress) with an always-on Cloud Run service that reaches
# the DB through the Cloud SQL Auth Connector unix socket.
#
# Usage:
#   bash scripts/deploy-cloudrun.sh [staging|prod]     # default: staging
#
# Non-interactive auth (workspace automation SA):
#   export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
#
# Secrets (GCP Secret Manager):
#   khala-live-hub-token              — shared LiveHub service bearer (the
#                                       append target's auth). Reused as-is.
#   khala-sync-capture-password       — the khala_capture DB role password.
#                                       Create it first (never printed):
#     printf '%s' "$PW" | gcloud secrets create khala-sync-capture-password \
#       --replication-policy=automatic --data-file=-
#
# Non-secret connection vars (set on the service):
#   PGHOST=/cloudsql/openagentsgemini:us-central1:khala-sync-pg   (connector dir)
#   PGUSER=khala_capture
#   PGDATABASE=khala_sync_<env>
#   KHALA_SYNC_HUB_APPEND_URL=https://<livehub>/append
#
# Deploy shape (deliberate — see src/server.ts module doc):
#   min=max=1            — SINGLETON daemon (LISTEN/NOTIFY = one session; a
#                          second instance only double-pushes, hub dedupes).
#   --no-cpu-throttling  — the daemon loop + LISTEN + poll timer must run
#                          between HTTP requests.
#   --add-cloudsql-instances — mounts the connector socket under /cloudsql.

ENVIRONMENT="${1:-staging}"
if [[ "$ENVIRONMENT" != "staging" && "$ENVIRONMENT" != "prod" ]]; then
  echo "usage: $0 [staging|prod]" >&2
  exit 1
fi

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="${KHALA_CAPTURE_PROJECT:-openagentsgemini}"
REGION="${KHALA_CAPTURE_REGION:-us-central1}"
INSTANCE="${KHALA_CAPTURE_SQL_INSTANCE:-openagentsgemini:us-central1:khala-sync-pg}"

if [[ "$ENVIRONMENT" == "prod" ]]; then
  SERVICE="${KHALA_CAPTURE_SERVICE:-khala-capture}"
  PGDATABASE="${KHALA_CAPTURE_PGDATABASE:-khala_sync_prod}"
  HUB_URL_DEFAULT="https://khala-live-hub-ezxz4mgdsq-uc.a.run.app/append"
else
  SERVICE="${KHALA_CAPTURE_SERVICE:-khala-capture-staging}"
  PGDATABASE="${KHALA_CAPTURE_PGDATABASE:-khala_sync_staging}"
  HUB_URL_DEFAULT="https://khala-live-hub-staging-ezxz4mgdsq-uc.a.run.app/append"
fi

PGHOST="/cloudsql/${INSTANCE}"
PGUSER="${KHALA_CAPTURE_PGUSER:-khala_capture}"
HUB_APPEND_URL="${KHALA_SYNC_HUB_APPEND_URL:-$HUB_URL_DEFAULT}"
TOKEN_SECRET="${KHALA_CAPTURE_HUB_TOKEN_SECRET:-khala-live-hub-token}"
PW_SECRET="${KHALA_CAPTURE_PGPASSWORD_SECRET:-khala-sync-capture-password}"

cd "$APP_DIR"

echo "==> Building Cloud Run server bundle"
bun run build:cloudrun

if ! gcloud secrets describe "$PW_SECRET" --project "$PROJECT" >/dev/null 2>&1; then
  echo "==> Secret $PW_SECRET is missing." >&2
  echo "    Create it first (the khala_capture DB role password; never printed):" >&2
  echo "      printf '%s' \"\$PW\" | gcloud secrets create $PW_SECRET --project $PROJECT --replication-policy=automatic --data-file=-" >&2
  exit 1
fi
if ! gcloud secrets describe "$TOKEN_SECRET" --project "$PROJECT" >/dev/null 2>&1; then
  echo "==> Secret $TOKEN_SECRET is missing (the LiveHub shared bearer)." >&2
  exit 1
fi

echo "==> Deploying $SERVICE to Cloud Run ($REGION, env $ENVIRONMENT)"
gcloud run deploy "$SERVICE" \
  --project "$PROJECT" \
  --source . \
  --region "$REGION" \
  --allow-unauthenticated \
  --port 8080 \
  --timeout 3600 \
  --min-instances 1 \
  --max-instances 1 \
  --concurrency 20 \
  --no-cpu-throttling \
  --cpu 1 \
  --memory 512Mi \
  --set-env-vars "PGHOST=${PGHOST},PGUSER=${PGUSER},PGDATABASE=${PGDATABASE},KHALA_SYNC_HUB_APPEND_URL=${HUB_APPEND_URL}" \
  --set-secrets "PGPASSWORD=${PW_SECRET}:latest,KHALA_SYNC_HUB_TOKEN=${TOKEN_SECRET}:latest" \
  --add-cloudsql-instances "$INSTANCE"

URL="$(gcloud run services describe "$SERVICE" --project "$PROJECT" --region "$REGION" --format='value(status.url)')"
echo "==> Done. Service URL: $URL"
echo "    Verify:"
echo "      curl -s $URL/health    # => {\"ok\":true,\"listener\":\"listening\",...}"
echo "      # then confirm checkpoints advance (docs/khala-sync/RUNBOOK.md liveness query)"
