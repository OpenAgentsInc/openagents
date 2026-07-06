#!/usr/bin/env bash
set -euo pipefail

# Deploy oa-queue-worker (Postgres JobQueue pump) to Cloud Run (CFG-7, #8522).
#
# Required before running:
#   gcloud auth login
#   gcloud config set project openagentsgemini
#
# Secrets (GCP Secret Manager, created out of band — never from tracked files):
#   oa-queue-worker-database-url    direct Postgres URL for oa_infra_jobs
#                                   (khala_app on the khala-sync Cloud SQL
#                                   instance; staging or prod database)
#   oa-queue-worker-delivery-token  admin bearer for the app's
#                                   /api/internal/queue/deliver route
#
# min-instances=1: the pump is a poller — Cloud Run must not scale it to
# zero or leased-job delivery stops. max-instances=1 keeps ordering pressure
# and connection count low; FOR UPDATE SKIP LOCKED makes >1 safe if ever
# needed.

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE="${OA_QUEUE_WORKER_SERVICE:-oa-queue-worker}"
REGION="${OA_QUEUE_WORKER_REGION:-us-central1}"
DB_SECRET="${OA_QUEUE_WORKER_DB_SECRET:-oa-queue-worker-database-url}"
TOKEN_SECRET="${OA_QUEUE_WORKER_TOKEN_SECRET:-oa-queue-worker-delivery-token}"
DELIVERY_URL="${OA_QUEUE_DELIVERY_URL:-https://openagents.com}"

cd "$APP_DIR"

echo "==> Building self-contained server bundle"
bun run build

echo "==> Deploying $SERVICE to Cloud Run ($REGION)"
gcloud run deploy "$SERVICE" \
  --source . \
  --region "$REGION" \
  --no-allow-unauthenticated \
  --port 8080 \
  --min-instances 1 \
  --max-instances 1 \
  --cpu 1 \
  --memory 512Mi \
  --set-env-vars "OA_QUEUE_DELIVERY_URL=${DELIVERY_URL}" \
  --set-secrets "OA_INFRA_DATABASE_URL=${DB_SECRET}:latest,OA_QUEUE_DELIVERY_TOKEN=${TOKEN_SECRET}:latest"

echo "==> Done. Verify (authenticated):"
echo "    gcloud run services proxy $SERVICE --region $REGION --port 9090 &"
echo "    curl -s localhost:9090/  # => { ok: true, cycles, lastCycleAt, ... }"
