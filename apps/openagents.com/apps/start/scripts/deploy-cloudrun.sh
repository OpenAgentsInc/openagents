#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-stage1}"
PROJECT="${OPENAGENTS_GCP_PROJECT:-openagentsgemini}"
REGION="${OPENAGENTS_GCP_REGION:-us-central1}"
PUBLIC_API_ORIGIN="${OPENAGENTS_PUBLIC_API_ORIGIN:-https://openagents.com}"
FORGE_READ_BASE_URL="${OPENAGENTS_FORGE_READ_BASE_URL:-https://forge-git-ezxz4mgdsq-uc.a.run.app}"
FORGE_SERVICE_AUTH_SECRET="${OPENAGENTS_FORGE_SERVICE_AUTH_SECRET:-openagents-forge-git-policy-authority-token}"
NETWORK="${OPENAGENTS_START_NETWORK:-default}"
SUBNETWORK="${OPENAGENTS_START_SUBNETWORK:-forge-git}"

case "$TARGET" in
  stage1)
    SERVICE="${OPENAGENTS_START_SERVICE:-openagents-com-start-stage1}"
    ;;
  *)
    echo "usage: $0 stage1" >&2
    exit 2
    ;;
esac

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$APP_DIR"
echo "==> Building TanStack Start app"
pnpm run build

echo "==> Deploying $SERVICE to Cloud Run ($REGION)"
gcloud run deploy "$SERVICE" \
  --project "$PROJECT" \
  --source . \
  --region "$REGION" \
  --allow-unauthenticated \
  --port 8080 \
  --min-instances 0 \
  --max-instances 2 \
  --cpu 1 \
  --memory 512Mi \
  --timeout 300 \
  --concurrency 40 \
  --network "$NETWORK" \
  --subnet "$SUBNETWORK" \
  --vpc-egress all-traffic \
  --set-env-vars "OPENAGENTS_PUBLIC_API_ORIGIN=${PUBLIC_API_ORIGIN},KHALA_SYNC_UPSTREAM_BASE_URL=${PUBLIC_API_ORIGIN},OPENAGENTS_FORGE_READ_BASE_URL=${FORGE_READ_BASE_URL}" \
  --set-secrets "OPENAGENTS_FORGE_GIT_SERVICE_AUTH_TOKEN=${FORGE_SERVICE_AUTH_SECRET}:latest"

SERVICE_URL="$(gcloud run services describe "$SERVICE" --project "$PROJECT" --region "$REGION" --format='value(status.url)')"
echo "==> Deployed: $SERVICE_URL"

echo "==> Smoke: /internal/healthz"
curl -fsS "${SERVICE_URL}/internal/healthz"
echo

echo "==> Smoke: /stage1"
curl -fsSI "${SERVICE_URL}/stage1" >/dev/null
echo "${SERVICE_URL}/stage1"
