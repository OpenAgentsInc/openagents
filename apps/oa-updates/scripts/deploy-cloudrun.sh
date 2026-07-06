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
#   export OA_DESKTOP_RELEASES_DIST=/app/desktop-dist
#
# Code signing (#8530 / CFG-14): the OTA manifest signing key reaches the
# service as the OA_SIGNING_KEY env var mounted from GCP Secret Manager
# (secret `oa-updates-codesign-key`, project openagentsgemini) via
# --set-secrets. It is never passed as inline env. To point at a different
# secret/version, export OA_SIGNING_SECRET=<secret-name>:<version>; set it
# to the empty string to deploy without code signing (dev projects only).
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

# OA_SIGNING_KEY is mounted from Secret Manager, never inline (#8530).
# --set-env-vars above replaces the full inline env list, so any previously
# inline OA_SIGNING_KEY is dropped by the same deploy that mounts the secret.
signing_secret="${OA_SIGNING_SECRET-oa-updates-codesign-key:latest}"
if [[ -n "$signing_secret" ]]; then
  args+=(--set-secrets "OA_SIGNING_KEY=${signing_secret}")
fi

gcloud "${args[@]}"
