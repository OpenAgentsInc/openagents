#!/usr/bin/env bash
set -euo pipefail

# Deploy the FORGE-02 stock Git revision to the Terraform-owned Cloud Run
# service. Terraform owns the service shell, NFS volume, VPC, and load-balancer
# path. This script changes only the service image and runtime configuration.

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="$(cd "$APP_DIR/../.." && pwd)"
PROJECT="${OPENAGENTS_GCP_PROJECT:-openagentsgemini}"
REGION="${OPENAGENTS_GCP_REGION:-us-central1}"
BUILD_SERVICE_ACCOUNT="projects/${PROJECT}/serviceAccounts/oa-cloud-run-source-builder@${PROJECT}.iam.gserviceaccount.com"
SERVICE="${FORGE_GIT_SERVICE:-forge-git}"
DATABASE_SECRET="${FORGE_GIT_DATABASE_SECRET:-openagents-monolith-database-url-prod}"
DATABASE_PASSWORD_SECRET="${FORGE_GIT_DATABASE_PASSWORD_SECRET:-openagents-monolith-pgpassword}"
POLICY_AUTHORITY_SECRET="${FORGE_GIT_POLICY_AUTHORITY_SECRET:-openagents-forge-git-policy-authority-token}"
# The deployed monolith already uses this separately scoped internal bearer for
# Forge service calls. The route accepts it only on the private mirror paths.
INTERNAL_SERVICE_AUTH_SECRET="${FORGE_GIT_INTERNAL_SERVICE_AUTH_SECRET:-openagents-forge-git-policy-authority-token}"
GITHUB_MIRROR_SSH_SECRET="${FORGE_GIT_GITHUB_MIRROR_SSH_SECRET:-openagents-forge-github-mirror-deploy-key}"
GITHUB_MIRROR_REPOSITORIES="${FORGE_GIT_GITHUB_MIRROR_ALLOWED_REPOSITORIES:-OpenAgentsInc/omega}"
PUBLIC_READ_REPOSITORIES="${FORGE_GIT_PUBLIC_READ_REPOSITORIES:-tenant.openagents/omega}"
POLICY_AUTHORITY_URL="${FORGE_GIT_POLICY_AUTHORITY_URL:-https://openagents.com/internal/forge/git-authorize}"
DATABASE_INSTANCE="${FORGE_GIT_DATABASE_INSTANCE:-openagentsgemini:us-central1:khala-sync-pg}"
ARTIFACTS_BUCKET="${FORGE_GIT_ARTIFACTS_BUCKET:-openagentsgemini-oa-artifacts}"
ARTIFACTS_PREFIX="${FORGE_GIT_ARTIFACTS_PREFIX:-forge/git-packs/}"
NFS_INSTANCE="${FORGE_GIT_NFS_INSTANCE:-forge-git-nfs}"
NFS_EXPORT="${FORGE_GIT_NFS_EXPORT:-/srv/forge/repositories}"
ZONE="${OPENAGENTS_GCP_ZONE:-us-central1-a}"
NETWORK="${FORGE_GIT_NETWORK:-default}"
SUBNETWORK="${FORGE_GIT_SUBNETWORK:-forge-git}"
RUNTIME_SERVICE_ACCOUNT="${FORGE_GIT_RUNTIME_SERVICE_ACCOUNT:-forge-git-runtime@openagentsgemini.iam.gserviceaccount.com}"

node "$REPO_ROOT/scripts/google-cloud-authority-guard.mjs"

cd "$APP_DIR"
pnpm run build

RUNTIME_DEPLOY_DIR="$(mktemp -d)"
trap 'rm -rf "$RUNTIME_DEPLOY_DIR" "$APP_DIR/dist/node_modules"' EXIT
(cd "$REPO_ROOT" && CI=true pnpm --config.ignore-scripts=true \
  --config.node-linker=hoisted \
  --config.allow-unused-patches=true \
  --filter @openagentsinc/forge-git-service deploy "$RUNTIME_DEPLOY_DIR" \
  --prod --legacy)
mv "$RUNTIME_DEPLOY_DIR/node_modules" "$APP_DIR/dist/node_modules"
(cd "$REPO_ROOT" && CI=true pnpm install --frozen-lockfile --ignore-scripts >/dev/null)

NFS_IP="$(
  gcloud compute instances describe "$NFS_INSTANCE" \
    --project "$PROJECT" \
    --zone "$ZONE" \
    --format='value(networkInterfaces[0].networkIP)'
)"

gcloud run deploy "$SERVICE" \
  --project "$PROJECT" \
  --region "$REGION" \
  --source . \
  --build-service-account "$BUILD_SERVICE_ACCOUNT" \
  --execution-environment gen2 \
  --service-account "$RUNTIME_SERVICE_ACCOUNT" \
  --ingress internal-and-cloud-load-balancing \
  --allow-unauthenticated \
  --port 8080 \
  --min 0 \
  --max 1 \
  --cpu 2 \
  --memory 2Gi \
  --concurrency 80 \
  --timeout 3600 \
  --network "$NETWORK" \
  --subnet "$SUBNETWORK" \
  --vpc-egress private-ranges-only \
  --network-tags forge-git \
  --add-cloudsql-instances "$DATABASE_INSTANCE" \
  --add-volume "name=forge-repositories,type=nfs,location=${NFS_IP}:${NFS_EXPORT}" \
  --add-volume-mount "volume=forge-repositories,mount-path=/var/lib/forge/repositories" \
  --set-env-vars "FORGE_GIT_REPOSITORY_ROOT=/var/lib/forge/repositories,FORGE_GIT_GCS_MIRROR_ENABLED=true,FORGE_GIT_POLICY_AUTHORITY_URL=${POLICY_AUTHORITY_URL},FORGE_GIT_GITHUB_MIRROR_ALLOWED_REPOSITORIES=${GITHUB_MIRROR_REPOSITORIES},FORGE_GIT_PUBLIC_READ_REPOSITORIES=${PUBLIC_READ_REPOSITORIES},FORGE_GIT_GITHUB_MIRROR_SSH_KEY_PATH=/tmp/forge-github-mirror-id_ed25519,OA_INFRA_GCS_BUCKET=${ARTIFACTS_BUCKET},OA_INFRA_GCS_PREFIX=${ARTIFACTS_PREFIX},PGHOST=/cloudsql/${DATABASE_INSTANCE},PGUSER=khala_app" \
  --set-secrets "FORGE_GIT_DATABASE_URL=${DATABASE_SECRET}:latest,FORGE_GIT_POLICY_AUTHORITY_TOKEN=${POLICY_AUTHORITY_SECRET}:latest,FORGE_GIT_INTERNAL_SERVICE_AUTH_TOKEN=${INTERNAL_SERVICE_AUTH_SECRET}:latest,FORGE_GIT_GITHUB_MIRROR_SSH_PRIVATE_KEY=${GITHUB_MIRROR_SSH_SECRET}:latest,PGPASSWORD=${DATABASE_PASSWORD_SECRET}:latest"

SERVICE_URL="$(
  gcloud run services describe "$SERVICE" \
    --project "$PROJECT" \
    --region "$REGION" \
    --format='value(status.url)'
)"

gcloud compute ssh "$NFS_INSTANCE" \
  --project "$PROJECT" \
  --zone "$ZONE" \
  --tunnel-through-iap \
  --command "curl -fsS '${SERVICE_URL}/internal/healthz'"
echo
