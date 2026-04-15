#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"
BUILD_CONTEXT_SCRIPT="${SCRIPT_DIR}/stage-build-context.sh"

require_cmd git
require_cmd gcloud
require_cmd python3

if [[ "${NEXUS_IMAGE_TAG}" == "latest" ]]; then
  NEXUS_IMAGE_TAG="$(git -C "$ROOT_DIR" rev-parse --short=12 HEAD)"
  export NEXUS_IMAGE_TAG
  export NEXUS_IMAGE="${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${NEXUS_ARTIFACT_REPO}/${NEXUS_IMAGE_NAME}:${NEXUS_IMAGE_TAG}"
fi

ensure_gcloud_context
ensure_services

if ! gcloud artifacts repositories describe "$NEXUS_ARTIFACT_REPO" \
  --project "$GCP_PROJECT" \
  --location "$GCP_REGION" >/dev/null 2>&1; then
  log "Creating Artifact Registry repository: ${NEXUS_ARTIFACT_REPO}"
  gcloud artifacts repositories create "$NEXUS_ARTIFACT_REPO" \
    --project "$GCP_PROJECT" \
    --location "$GCP_REGION" \
    --repository-format docker \
    --description "OpenAgents Nexus relay images" >/dev/null
fi

if [[ "${NEXUS_BUILD_SCCACHE_ENABLED}" == "true" ]]; then
  if ! gcloud storage buckets describe "gs://${NEXUS_BUILD_SCCACHE_BUCKET}" --project "$GCP_PROJECT" >/dev/null 2>&1; then
    log "Creating GCS bucket for Nexus sccache: gs://${NEXUS_BUILD_SCCACHE_BUCKET}"
    gcloud storage buckets create "gs://${NEXUS_BUILD_SCCACHE_BUCKET}" \
      --project "$GCP_PROJECT" \
      --location "$GCP_REGION" \
      --uniform-bucket-level-access >/dev/null
  fi

  PROJECT_NUMBER="$(gcloud projects describe "$GCP_PROJECT" --format='value(projectNumber)')"
  CLOUD_BUILD_SERVICE_ACCOUNT="${PROJECT_NUMBER}@cloudbuild.gserviceaccount.com"
  log "Ensuring Cloud Build bucket access for ${CLOUD_BUILD_SERVICE_ACCOUNT}"
  gcloud storage buckets add-iam-policy-binding "gs://${NEXUS_BUILD_SCCACHE_BUCKET}" \
    --project "$GCP_PROJECT" \
    --member "serviceAccount:${CLOUD_BUILD_SERVICE_ACCOUNT}" \
    --role roles/storage.objectAdmin >/dev/null
fi

SCCACHE_BUCKET_SUBSTITUTION=""
SCCACHE_KEY_PREFIX_SUBSTITUTION=""
if [[ "${NEXUS_BUILD_SCCACHE_ENABLED}" == "true" ]]; then
  SCCACHE_BUCKET_SUBSTITUTION="${NEXUS_BUILD_SCCACHE_BUCKET}"
  SCCACHE_KEY_PREFIX_SUBSTITUTION="${NEXUS_BUILD_SCCACHE_KEY_PREFIX}"
fi

TMP_BUILD_CONTEXT="$(mktemp -d "${TMPDIR:-/tmp}/openagents-nexus-build-context.XXXXXX")"
trap 'rm -rf "$TMP_BUILD_CONTEXT"' EXIT
bash "$BUILD_CONTEXT_SCRIPT" "$TMP_BUILD_CONTEXT" >/dev/null

BUILD_CONTEXT_FILE_COUNT="$(find "$TMP_BUILD_CONTEXT" -type f | wc -l | tr -d '[:space:]')"
BUILD_CONTEXT_SIZE="$(du -sh "$TMP_BUILD_CONTEXT" | awk '{print $1}')"

log "Building and pushing image: ${NEXUS_IMAGE}"
log "Submitting Nexus-only build context: files=${BUILD_CONTEXT_FILE_COUNT} size=${BUILD_CONTEXT_SIZE} profile=${NEXUS_BUILD_PROFILE} timeout=${NEXUS_BUILD_TIMEOUT}"
gcloud builds submit "$TMP_BUILD_CONTEXT" \
  --project "$GCP_PROJECT" \
  --timeout "${NEXUS_BUILD_TIMEOUT}" \
  --config "${ROOT_DIR}/apps/nexus-relay/deploy/cloudbuild.yaml" \
  --substitutions "_IMAGE=${NEXUS_IMAGE},_CACHE_IMAGE=${NEXUS_BUILD_CACHE_IMAGE},_BUILD_PROFILE=${NEXUS_BUILD_PROFILE},_SCCACHE_BUCKET=${SCCACHE_BUCKET_SUBSTITUTION},_SCCACHE_KEY_PREFIX=${SCCACHE_KEY_PREFIX_SUBSTITUTION}"

LATEST_IMAGE="${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${NEXUS_ARTIFACT_REPO}/${NEXUS_IMAGE_NAME}:latest"
if [[ "${NEXUS_BUILD_UPDATE_LATEST_TAG}" == "true" ]]; then
  log "Tagging ${NEXUS_IMAGE} as ${LATEST_IMAGE}"
  gcloud artifacts docker tags add \
    "$NEXUS_IMAGE" \
    "$LATEST_IMAGE" \
    --project "$GCP_PROJECT" >/dev/null
else
  log "Skipping latest tag update for validation build"
fi

log "Build complete"
log "NEXUS_IMAGE=${NEXUS_IMAGE}"
log "NEXUS_BUILD_CACHE_IMAGE=${NEXUS_BUILD_CACHE_IMAGE}"
