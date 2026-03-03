#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd git
require_cmd gcloud

SYMPHONY_SRC="${SYMPHONY_SRC:-/Users/christopherdavid/code/maestro/maestro-symphony}"

if [[ ! -d "$SYMPHONY_SRC/.git" ]]; then
  die "SYMPHONY_SRC does not look like a git checkout: ${SYMPHONY_SRC}"
fi

if [[ "${SYMPHONY_IMAGE_TAG}" == "latest" ]]; then
  SYMPHONY_IMAGE_TAG="$(git -C "$SYMPHONY_SRC" rev-parse --short=12 HEAD)"
  export SYMPHONY_IMAGE_TAG
  export SYMPHONY_IMAGE="${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${SYMPHONY_ARTIFACT_REPO}/${SYMPHONY_IMAGE_NAME}:${SYMPHONY_IMAGE_TAG}"
fi

ensure_gcloud_context
ensure_services

if ! gcloud artifacts repositories describe "$SYMPHONY_ARTIFACT_REPO" \
  --project "$GCP_PROJECT" \
  --location "$GCP_REGION" >/dev/null 2>&1; then
  log "Creating Artifact Registry repository: ${SYMPHONY_ARTIFACT_REPO}"
  gcloud artifacts repositories create "$SYMPHONY_ARTIFACT_REPO" \
    --project "$GCP_PROJECT" \
    --location "$GCP_REGION" \
    --repository-format docker \
    --description "Maestro Symphony container images" >/dev/null
fi

log "Building and pushing image: ${SYMPHONY_IMAGE}"
TMP_CLOUDBUILD_FILE="$(mktemp)"
trap 'rm -f "$TMP_CLOUDBUILD_FILE"' EXIT

cat >"$TMP_CLOUDBUILD_FILE" <<'YAML'
steps:
  - name: gcr.io/cloud-builders/docker
    env:
      - DOCKER_BUILDKIT=1
    args:
      - buildx
      - build
      - --platform
      - linux/amd64
      - --build-arg
      - TARGETARCH=amd64
      - -t
      - ${_IMAGE}
      - --push
      - .
YAML

gcloud builds submit "$SYMPHONY_SRC" \
  --project "$GCP_PROJECT" \
  --config "$TMP_CLOUDBUILD_FILE" \
  --substitutions "_IMAGE=${SYMPHONY_IMAGE}"

LATEST_IMAGE="${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${SYMPHONY_ARTIFACT_REPO}/${SYMPHONY_IMAGE_NAME}:latest"
log "Tagging ${SYMPHONY_IMAGE} as ${LATEST_IMAGE}"
gcloud artifacts docker tags add \
  "$SYMPHONY_IMAGE" \
  "$LATEST_IMAGE" \
  --project "$GCP_PROJECT" >/dev/null

log "Build complete"
log "SYMPHONY_IMAGE=${SYMPHONY_IMAGE}"
