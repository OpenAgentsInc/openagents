#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
PROJECT="openagentsgemini"
REGION="us-central1"
SERVICE="openagents-docs"

node "$REPO_ROOT/scripts/google-cloud-authority-guard.mjs"

cd "$SCRIPT_DIR"
echo "==> Building strict static Blume docs artifact"
pnpm run build

echo "==> Deploying $SERVICE to Cloud Run ($REGION)"
gcloud run deploy "$SERVICE" \
  --project "$PROJECT" \
  --region "$REGION" \
  --source . \
  --allow-unauthenticated \
  --ingress all \
  --cpu 1 \
  --memory 256Mi \
  --min 1 \
  --max 4 \
  --timeout 30 \
  --quiet

SERVICE_URL="$(gcloud run services describe "$SERVICE" --project "$PROJECT" --region "$REGION" --format='value(status.url)')"
curl -fsS "$SERVICE_URL/internal/healthz" >/dev/null
curl -fsS "$SERVICE_URL/docs" >/dev/null
echo "==> Deployed $SERVICE_URL"
