#!/usr/bin/env bash
set -euo pipefail
APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="${GOOGLE_CLOUD_PROJECT:-openagentsgemini}"
REGION="${OPENAGENTS_AUDIO_RUN_REGION:-us-central1}"
SERVICE="${OPENAGENTS_AUDIO_SERVICE:-openagents-audio-staging}"
SECRET="${OPENAGENTS_AUDIO_TOKEN_SECRET_NAME:-openagents-audio-token-secret}"
cd "$APP_DIR"
bun run build:cloudrun
if ! gcloud secrets describe "$SECRET" --project "$PROJECT" >/dev/null 2>&1; then
  openssl rand -hex 32 | tr -d '\n' | gcloud secrets create "$SECRET" --project "$PROJECT" --replication-policy=automatic --data-file=-
fi
gcloud run deploy "$SERVICE" --project "$PROJECT" --region "$REGION" --source . \
  --no-allow-unauthenticated --port 8080 --timeout 3600 --session-affinity \
  --min-instances 0 --max-instances 20 --concurrency 100 --memory 1Gi \
  --set-env-vars "GOOGLE_CLOUD_PROJECT=${PROJECT},OPENAGENTS_AUDIO_STT_LOCATION=us,OPENAGENTS_AUDIO_LANGUAGE=en-US" \
  --set-secrets "OPENAGENTS_AUDIO_TOKEN_SECRET=${SECRET}:latest"
