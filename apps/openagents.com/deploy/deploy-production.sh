#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

PROJECT="${PROJECT:-$(gcloud config get-value project 2>/dev/null)}"
REGION="${REGION:-us-central1}"
SERVICE="${SERVICE:-openagents-web}"
TAG="${TAG:-$(git -C "${APP_DIR}" rev-parse --short HEAD)}"
IMAGE="us-central1-docker.pkg.dev/${PROJECT}/openagents-web/laravel:${TAG}"

if [[ -z "${PROJECT}" || "${PROJECT}" == "(unset)" ]]; then
  echo "error: PROJECT is unset and no gcloud default project is configured" >&2
  exit 1
fi

echo "[deploy] project=${PROJECT} region=${REGION} service=${SERVICE} tag=${TAG}"
echo "[deploy] building image via Cloud Build (Dockerfile runs npm run build)"
gcloud builds submit \
  --project "${PROJECT}" \
  --config "${APP_DIR}/deploy/cloudbuild.yaml" \
  --substitutions "_TAG=${TAG}" \
  "${APP_DIR}"

echo "[deploy] deploying image=${IMAGE}"
gcloud run deploy "${SERVICE}" \
  --project "${PROJECT}" \
  --region "${REGION}" \
  --image "${IMAGE}"

if [[ "${SYNC_DOCS_OPENAPI:-1}" == "1" ]]; then
  echo "[deploy] syncing OpenAPI spec into docs repo"
  "${APP_DIR}/deploy/sync-openapi-to-docs.sh"
else
  echo "[deploy] SYNC_DOCS_OPENAPI=0; skipping docs sync"
fi

echo "[deploy] complete"
