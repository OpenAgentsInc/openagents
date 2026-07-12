#!/usr/bin/env bash
set -euo pipefail
PROJECT="${GOOGLE_CLOUD_PROJECT:-openagentsgemini}"
REGION="${OPENAGENTS_AUDIO_RUN_REGION:-us-central1}"
SERVICE="${OPENAGENTS_AUDIO_EDGE_SERVICE:-openagents-audio-edge-staging}"
IDENTITY="${OPENAGENTS_AUDIO_EDGE_SERVICE_ACCOUNT:-openagents-audio-edge@${PROJECT}.iam.gserviceaccount.com}"
UPSTREAM="${OPENAGENTS_AUDIO_CLOUD_RUN_URL:-https://openagents-audio-staging-157437760789.us-central1.run.app}"
gcloud run deploy "$SERVICE" --project "$PROJECT" --region "$REGION" --source "$(cd "$(dirname "$0")" && pwd)" \
  --allow-unauthenticated --port 8080 --timeout 3600 --session-affinity --min-instances 0 --max-instances 20 \
  --concurrency 100 --memory 256Mi --service-account "$IDENTITY" --set-env-vars "OPENAGENTS_AUDIO_CLOUD_RUN_URL=${UPSTREAM}"
