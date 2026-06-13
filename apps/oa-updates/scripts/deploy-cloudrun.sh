#!/usr/bin/env bash
set -euo pipefail

# Deploy OpenAgents Updates to Cloud Run from the oa-updates app directory.
#
# Required before running:
#   gcloud auth login
#   gcloud config set project <project-id>
#   export OA_PUBLIC_URL=https://<your-cloud-run-service-url>
#   export OA_SEED_DIST=/workspace/dist
#   export OA_SEED_RUNTIME=<runtime-version>
#
# Optional:
#   export OA_SEED_PLATFORM=ios
#   export OA_SIGNING_KEY="$(cat private-key.pem)"
#
# This script is intentionally not run by tests or setup. Run it manually when
# the target Google Cloud project and seed export are ready.

args=(
  run deploy oa-updates
  --source apps/oa-updates \
  --region us-central1 \
  --allow-unauthenticated \
  --port 8080 \
  --set-env-vars "OA_PUBLIC_URL=${OA_PUBLIC_URL:?set OA_PUBLIC_URL},OA_SEED_DIST=${OA_SEED_DIST:?set OA_SEED_DIST},OA_SEED_RUNTIME=${OA_SEED_RUNTIME:?set OA_SEED_RUNTIME},OA_SEED_PLATFORM=${OA_SEED_PLATFORM:-ios}"
)

if [[ -n "${OA_SIGNING_KEY:-}" ]]; then
  args+=(--set-env-vars "OA_SIGNING_KEY=${OA_SIGNING_KEY}")
fi

gcloud "${args[@]}"
