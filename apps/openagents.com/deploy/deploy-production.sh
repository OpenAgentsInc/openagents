#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(git -C "${APP_DIR}" rev-parse --show-toplevel)"

PROJECT="${PROJECT:-$(gcloud config get-value project 2>/dev/null)}"
REGION="${REGION:-us-central1}"
SERVICE="${SERVICE:-openagents-web}"
TAG="${TAG:-$(git -C "${APP_DIR}" rev-parse --short HEAD)}"
IMAGE="us-central1-docker.pkg.dev/${PROJECT}/openagents-web/laravel:${TAG}"

if [[ -z "${PROJECT}" || "${PROJECT}" == "(unset)" ]]; then
  echo "error: PROJECT is unset and no gcloud default project is configured" >&2
  exit 1
fi

# Ensure package-lock.json is in sync with package.json so Cloud Build's `npm ci` succeeds.
echo "[deploy] ensuring package-lock.json is in sync (npm install)"
(cd "${APP_DIR}" && npm install --no-audit --no-fund)

BUILD_CONTEXT="$(mktemp -d)"
trap 'rm -rf "${BUILD_CONTEXT}"' EXIT

echo "[deploy] preparing isolated build context with khala-sync package"
rsync -a \
  --exclude node_modules \
  --exclude vendor \
  "${APP_DIR}/" "${BUILD_CONTEXT}/"
mkdir -p "${BUILD_CONTEXT}/packages/khala-sync"
rsync -a \
  --exclude node_modules \
  "${REPO_ROOT}/packages/khala-sync/" "${BUILD_CONTEXT}/packages/khala-sync/"

echo "[deploy] project=${PROJECT} region=${REGION} service=${SERVICE} tag=${TAG}"
echo "[deploy] building image via Cloud Build (Dockerfile runs npm run build)"
gcloud builds submit \
  --project "${PROJECT}" \
  --config "${APP_DIR}/deploy/cloudbuild.yaml" \
  --substitutions "_TAG=${TAG}" \
  "${BUILD_CONTEXT}"

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
