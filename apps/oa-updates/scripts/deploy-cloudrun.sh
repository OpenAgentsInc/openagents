#!/usr/bin/env bash
set -euo pipefail

# Deploy OpenAgents Updates to Cloud Run from the oa-updates app directory.
#
# Required before running:
#   gcloud auth login
#   gcloud config set project <project-id>
#   export OA_PUBLIC_URL=https://<your-cloud-run-service-url>
#   export OA_SEED_DIST=/app/dist
#   export OA_SEED_RUNTIME=<runtime-version>
#
# Optional:
#   export OA_SEED_PLATFORM=ios
#   export OA_SEED_EXPO_CLIENT_PATH=/app/dist/expo-client.json
#   export OA_SIGNING_KEY="$(cat private-key.pem)"
#   export OA_DESKTOP_RELEASES_DIST=/app/desktop-dist
#
# This script is intentionally not run by tests or setup. Run it manually when
# the target Google Cloud project and seed export are ready.

env_vars=("OA_PUBLIC_URL=${OA_PUBLIC_URL:?set OA_PUBLIC_URL}")

if [[ -n "${OA_SEED_DIST:-}" || -n "${OA_SEED_RUNTIME:-}" ]]; then
  env_vars+=(
    "OA_SEED_DIST=${OA_SEED_DIST:?set OA_SEED_DIST}"
    "OA_SEED_RUNTIME=${OA_SEED_RUNTIME:?set OA_SEED_RUNTIME}"
    "OA_SEED_PLATFORM=${OA_SEED_PLATFORM:-ios}"
  )

  if [[ -n "${OA_SEED_EXPO_CLIENT_PATH:-}" ]]; then
    env_vars+=("OA_SEED_EXPO_CLIENT_PATH=${OA_SEED_EXPO_CLIENT_PATH}")
  fi
fi

if [[ -n "${OA_DESKTOP_RELEASES_DIST:-}" ]]; then
  env_vars+=("OA_DESKTOP_RELEASES_DIST=${OA_DESKTOP_RELEASES_DIST}")
fi

env_csv="$(IFS=,; echo "${env_vars[*]}")"

args=(
  run deploy oa-updates
  --source apps/oa-updates \
  --region us-central1 \
  --allow-unauthenticated \
  --port 8080 \
  --set-env-vars "$env_csv"
)

if [[ -n "${OA_SIGNING_KEY:-}" ]]; then
  args+=(--set-env-vars "OA_SIGNING_KEY=${OA_SIGNING_KEY}")
fi

gcloud "${args[@]}"
