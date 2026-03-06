#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd git
require_cmd gcloud

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

log "Building and pushing image: ${NEXUS_IMAGE}"
gcloud builds submit "$ROOT_DIR" \
  --project "$GCP_PROJECT" \
  --config "${ROOT_DIR}/apps/nexus-relay/deploy/cloudbuild.yaml" \
  --substitutions "_IMAGE=${NEXUS_IMAGE}"

LATEST_IMAGE="${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${NEXUS_ARTIFACT_REPO}/${NEXUS_IMAGE_NAME}:latest"
log "Tagging ${NEXUS_IMAGE} as ${LATEST_IMAGE}"
gcloud artifacts docker tags add \
  "$NEXUS_IMAGE" \
  "$LATEST_IMAGE" \
  --project "$GCP_PROJECT" >/dev/null

log "Build complete"
log "NEXUS_IMAGE=${NEXUS_IMAGE}"
