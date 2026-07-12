#!/usr/bin/env bash
set -euo pipefail
APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="${GOOGLE_CLOUD_PROJECT:-openagentsgemini}"
REGION="${OPENAGENTS_AUDIO_RUN_REGION:-us-central1}"
SERVICE="${OPENAGENTS_AUDIO_SERVICE:-openagents-audio-staging}"
SECRET="${OPENAGENTS_AUDIO_TOKEN_SECRET_NAME:-openagents-audio-token-secret}"
ENCRYPTION_SECRET="${OPENAGENTS_AUDIO_ENCRYPTION_SECRET_NAME:-openagents-audio-encryption-key}"
DATABASE_SECRET="${OPENAGENTS_AUDIO_DATABASE_SECRET_NAME:-openagents-audio-database-url-staging}"
RETENTION_BUCKET="${OPENAGENTS_AUDIO_RETENTION_BUCKET:-openagents-audio-retention-staging-157437760789}"
TTS_VOICE="${OPENAGENTS_AUDIO_TTS_VOICE:-en-US-Chirp3-HD-Sulafat}"
SERVICE_ACCOUNT="${OPENAGENTS_AUDIO_SERVICE_ACCOUNT:-oa-audio-retention@${PROJECT}.iam.gserviceaccount.com}"
SQL_INSTANCE="${OPENAGENTS_AUDIO_CLOUD_SQL_INSTANCE:-${PROJECT}:${REGION}:khala-sync-pg}"
cd "$APP_DIR"
bun run build:cloudrun
if ! gcloud secrets describe "$SECRET" --project "$PROJECT" >/dev/null 2>&1; then
  openssl rand -hex 32 | tr -d '\n' | gcloud secrets create "$SECRET" --project "$PROJECT" --replication-policy=automatic --data-file=-
fi
gcloud run deploy "$SERVICE" --project "$PROJECT" --region "$REGION" --source . \
  --no-allow-unauthenticated --port 8080 --timeout 3600 --session-affinity \
  --min-instances 0 --max-instances 20 --concurrency 100 --memory 1Gi \
  --service-account "$SERVICE_ACCOUNT" --add-cloudsql-instances "$SQL_INSTANCE" \
  --set-env-vars "GOOGLE_CLOUD_PROJECT=${PROJECT},OPENAGENTS_AUDIO_STT_LOCATION=us,OPENAGENTS_AUDIO_LANGUAGE=en-US,OPENAGENTS_AUDIO_RETENTION_BUCKET=${RETENTION_BUCKET},OPENAGENTS_AUDIO_KEY_EPOCH=epoch:1,OPENAGENTS_AUDIO_TTS_VOICE=${TTS_VOICE}" \
  --set-secrets "OPENAGENTS_AUDIO_TOKEN_SECRET=${SECRET}:latest,OPENAGENTS_AUDIO_ENCRYPTION_KEY_BASE64=${ENCRYPTION_SECRET}:latest,OPENAGENTS_AUDIO_DATABASE_URL=${DATABASE_SECRET}:latest"
