#!/usr/bin/env bash
set -euo pipefail

# Deploy Aiur (owner-only admin panel) to Cloud Run (CFG-11, #8526).
#
# Required before running:
#   gcloud auth login
#   gcloud config set project openagentsgemini   # or your project
#
# The owner allowlist lives in GCP Secret Manager as `aiur-owner-user-ids`
# and is mounted as the AIUR_OWNER_USER_IDS env var. FAIL-CLOSED: if the
# secret is missing or empty the owner gate denies every request. The value
# must be exactly `github:14167547` (the owner) — never add anyone else
# without the owner explicitly asking.

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE="${AIUR_CLOUDRUN_SERVICE:-openagents-aiur}"
REGION="${AIUR_CLOUDRUN_REGION:-us-central1}"
SECRET_NAME="${AIUR_OWNER_SECRET_NAME:-aiur-owner-user-ids}"
OWNER_USER_IDS="github:14167547"

cd "$APP_DIR"

echo "==> Building client + Cloud Run server bundle"
bun run build:cloudrun

if ! gcloud secrets describe "$SECRET_NAME" >/dev/null 2>&1; then
  echo "==> Creating Secret Manager secret $SECRET_NAME"
  printf '%s' "$OWNER_USER_IDS" | gcloud secrets create "$SECRET_NAME" \
    --replication-policy=automatic \
    --data-file=-
else
  echo "==> Secret $SECRET_NAME already exists (leaving versions untouched)"
fi

echo "==> Deploying $SERVICE to Cloud Run ($REGION)"
gcloud run deploy "$SERVICE" \
  --source . \
  --region "$REGION" \
  --allow-unauthenticated \
  --port 8080 \
  --timeout 3600 \
  --set-env-vars "OPENAUTH_CLIENT_ID=openagents-web,OPENAUTH_ISSUER_URL=https://auth.openagents.com,KHALA_SYNC_UPSTREAM_BASE_URL=https://openagents.com" \
  --set-secrets "AIUR_OWNER_USER_IDS=${SECRET_NAME}:latest"

echo "==> Done. Verify:"
echo "    curl -s https://<service-url>/api/aiur/access   # => {\"kind\":\"signed_out\"}"
echo "    curl -s -o /dev/null -w '%{http_code}' -X POST https://<service-url>/api/sync/bootstrap -d '{}'  # => 401 (fail closed)"
