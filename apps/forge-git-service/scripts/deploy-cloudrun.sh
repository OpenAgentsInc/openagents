#!/usr/bin/env bash
set -euo pipefail

# Deploy the FORGE-02 stock Git revision to the Terraform-owned Cloud Run
# service. Terraform owns the service shell, NFS volume, VPC, and load-balancer
# path. This script changes only the service image and runtime configuration.

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$APP_DIR/../.." && pwd)"
PROJECT="${OPENAGENTS_GCP_PROJECT:-openagentsgemini}"
REGION="${OPENAGENTS_GCP_REGION:-us-central1}"
SERVICE="${FORGE_GIT_SERVICE:-forge-git}"
DATABASE_SECRET="${FORGE_GIT_DATABASE_SECRET:-openagents-monolith-database-url-prod}"
ARTIFACTS_BUCKET="${FORGE_GIT_ARTIFACTS_BUCKET:-openagentsgemini-oa-artifacts}"
ARTIFACTS_PREFIX="${FORGE_GIT_ARTIFACTS_PREFIX:-forge/git-packs}"
FILESTORE="${FORGE_GIT_FILESTORE:-forge-git-repositories}"
SHARE="${FORGE_GIT_FILESTORE_SHARE:-forge_repositories}"
NETWORK="${FORGE_GIT_NETWORK:-default}"
SUBNETWORK="${FORGE_GIT_SUBNETWORK:-default}"
RUNTIME_SERVICE_ACCOUNT="${FORGE_GIT_RUNTIME_SERVICE_ACCOUNT:-forge-git-runtime@openagentsgemini.iam.gserviceaccount.com}"

node "$REPO_ROOT/scripts/google-cloud-authority-guard.mjs"

cd "$APP_DIR"
pnpm run build

FILESTORE_IP="$(
  gcloud filestore instances describe "$FILESTORE" \
    --project "$PROJECT" \
    --zone "${FORGE_GIT_FILESTORE_ZONE:-us-central1-a}" \
    --format='value(networks.ipAddresses[0])'
)"

gcloud run deploy "$SERVICE" \
  --project "$PROJECT" \
  --region "$REGION" \
  --source . \
  --execution-environment gen2 \
  --service-account "$RUNTIME_SERVICE_ACCOUNT" \
  --ingress internal-and-cloud-load-balancing \
  --allow-unauthenticated \
  --port 8080 \
  --min 0 \
  --max 1 \
  --cpu 2 \
  --memory 2Gi \
  --concurrency 40 \
  --timeout 3600 \
  --network "$NETWORK" \
  --subnet "$SUBNETWORK" \
  --vpc-egress private-ranges-only \
  --network-tags forge-git \
  --add-volume "name=forge-repositories,type=nfs,location=${FILESTORE_IP}:/${SHARE}" \
  --add-volume-mount "volume=forge-repositories,mount-path=/var/lib/forge/repositories" \
  --set-env-vars "FORGE_GIT_REPOSITORY_ROOT=/var/lib/forge/repositories,FORGE_GIT_GCS_MIRROR_ENABLED=true,OA_INFRA_GCS_BUCKET=${ARTIFACTS_BUCKET},OA_INFRA_GCS_PREFIX=${ARTIFACTS_PREFIX}" \
  --set-secrets "FORGE_GIT_DATABASE_URL=${DATABASE_SECRET}:latest"

SERVICE_URL="$(
  gcloud run services describe "$SERVICE" \
    --project "$PROJECT" \
    --region "$REGION" \
    --format='value(status.url)'
)"

curl -fsS "${SERVICE_URL}/internal/healthz"
echo
