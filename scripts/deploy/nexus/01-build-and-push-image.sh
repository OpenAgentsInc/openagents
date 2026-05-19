#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"
BUILD_CONTEXT_SCRIPT="${SCRIPT_DIR}/stage-build-context.sh"
LDK_DEPLOY_INVARIANTS_SCRIPT="${SCRIPT_DIR}/test-ldk-deploy-invariants.sh"

require_cmd git
require_cmd gcloud
require_cmd python3
require_cmd jq

[[ -f "$LDK_DEPLOY_INVARIANTS_SCRIPT" ]] || die "Missing LDK deploy invariant guard: ${LDK_DEPLOY_INVARIANTS_SCRIPT}"
bash "$LDK_DEPLOY_INVARIANTS_SCRIPT" >/dev/null

if [[ "${NEXUS_IMAGE_TAG}" == "latest" ]]; then
  NEXUS_IMAGE_TAG="$(git -C "$ROOT_DIR" rev-parse --short=12 HEAD)"
  export NEXUS_IMAGE_TAG
  export NEXUS_IMAGE="${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${NEXUS_ARTIFACT_REPO}/${NEXUS_IMAGE_NAME}:${NEXUS_IMAGE_TAG}"
fi

REPORT_DIR="${ROOT_DIR}/docs/reports/nexus"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
GIT_SHORT_SHA="$(git -C "$ROOT_DIR" rev-parse --short=12 HEAD)"
BUILD_RECEIPT_PATH="${REPORT_DIR}/${STAMP}-cloudbuild-image-${GIT_SHORT_SHA}.json"

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
SCCACHE_ENABLED_JSON="false"
if [[ "${NEXUS_BUILD_SCCACHE_ENABLED}" == "true" ]]; then
  SCCACHE_ENABLED_JSON="true"
fi
SCCACHE_BUCKET_JSON="null"
SCCACHE_KEY_PREFIX_JSON="null"
if [[ -n "$SCCACHE_BUCKET_SUBSTITUTION" ]]; then
  SCCACHE_BUCKET_JSON="$(jq -Rn --arg value "$SCCACHE_BUCKET_SUBSTITUTION" '$value')"
fi
if [[ -n "$SCCACHE_KEY_PREFIX_SUBSTITUTION" ]]; then
  SCCACHE_KEY_PREFIX_JSON="$(jq -Rn --arg value "$SCCACHE_KEY_PREFIX_SUBSTITUTION" '$value')"
fi

TMP_BUILD_CONTEXT="$(mktemp -d "${TMPDIR:-/tmp}/openagents-nexus-build-context.XXXXXX")"
BUILD_RESULT_JSON_PATH="$(mktemp "${TMPDIR:-/tmp}/openagents-nexus-build-result.XXXXXX")"
trap 'rm -rf "$TMP_BUILD_CONTEXT" "$BUILD_RESULT_JSON_PATH"' EXIT
bash "$BUILD_CONTEXT_SCRIPT" "$TMP_BUILD_CONTEXT" >/dev/null

if command -v xattr >/dev/null 2>&1; then
  xattr -rc "$TMP_BUILD_CONTEXT" 2>/dev/null || true
fi

BUILD_CONTEXT_FILE_COUNT="$(find "$TMP_BUILD_CONTEXT" -type f | wc -l | tr -d '[:space:]')"
BUILD_CONTEXT_SIZE="$(du -sh "$TMP_BUILD_CONTEXT" | awk '{print $1}')"

log "Building and pushing image: ${NEXUS_IMAGE}"
log "Submitting Nexus-only build context: files=${BUILD_CONTEXT_FILE_COUNT} size=${BUILD_CONTEXT_SIZE} profile=${NEXUS_BUILD_PROFILE} timeout=${NEXUS_BUILD_TIMEOUT} machine_type=${NEXUS_BUILD_MACHINE_TYPE} disk_gb=${NEXUS_BUILD_DISK_SIZE_GB} sccache=${NEXUS_BUILD_SCCACHE_ENABLED}"
BUILD_STARTED_MS="$(timestamp_unix_ms)"
SUBMIT_OUTPUT="$(
gcloud builds submit "$TMP_BUILD_CONTEXT" \
  --project "$GCP_PROJECT" \
  --async \
  --machine-type "${NEXUS_BUILD_MACHINE_TYPE}" \
  --disk-size "${NEXUS_BUILD_DISK_SIZE_GB}" \
  --timeout "${NEXUS_BUILD_TIMEOUT}" \
  --format='value(id)' \
  --config "${ROOT_DIR}/apps/nexus-relay/deploy/cloudbuild.yaml" \
  --substitutions "_IMAGE=${NEXUS_IMAGE},_CACHE_IMAGE=${NEXUS_BUILD_CACHE_IMAGE},_BUILD_PROFILE=${NEXUS_BUILD_PROFILE},_SOURCE_REV=$(git -C "$ROOT_DIR" rev-parse HEAD),_SCCACHE_BUCKET=${SCCACHE_BUCKET_SUBSTITUTION},_SCCACHE_KEY_PREFIX=${SCCACHE_KEY_PREFIX_SUBSTITUTION}" \
  2>&1
)"

BUILD_ID="$(printf '%s\n' "$SUBMIT_OUTPUT" | grep -Eo '[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' | tail -n 1)"

[[ -n "$BUILD_ID" ]] || die "Cloud Build did not return a build id"

gcloud builds log --stream "$BUILD_ID" --project "$GCP_PROJECT" || true
BUILD_FINISHED_MS="$(timestamp_unix_ms)"
gcloud builds describe "$BUILD_ID" --project "$GCP_PROJECT" --format=json > "$BUILD_RESULT_JSON_PATH"

mkdir -p "$REPORT_DIR"
python3 - "$BUILD_RECEIPT_PATH" "$BUILD_RESULT_JSON_PATH" <<PY
import json
import sys
from pathlib import Path

receipt_path = Path(sys.argv[1])
build_result_json_path = Path(sys.argv[2])
receipt = {
    "generated_at": "${STAMP}",
    "kind": "nexus_cloudbuild_image",
    "git_sha": "$(git -C "$ROOT_DIR" rev-parse HEAD)",
    "git_short_sha": "${GIT_SHORT_SHA}",
    "source_rev_build_arg": "$(git -C "$ROOT_DIR" rev-parse HEAD)",
    "image": "${NEXUS_IMAGE}",
    "cache_image": "${NEXUS_BUILD_CACHE_IMAGE}",
    "build_profile": "${NEXUS_BUILD_PROFILE}",
    "machine_type": "${NEXUS_BUILD_MACHINE_TYPE}",
    "disk_size_gb": int("${NEXUS_BUILD_DISK_SIZE_GB}"),
    "sccache_enabled": json.loads("""${SCCACHE_ENABLED_JSON}"""),
    "sccache_bucket": json.loads("""${SCCACHE_BUCKET_JSON}"""),
    "sccache_key_prefix": json.loads("""${SCCACHE_KEY_PREFIX_JSON}"""),
    "context": {
        "file_count": int("${BUILD_CONTEXT_FILE_COUNT}"),
        "size_human": "${BUILD_CONTEXT_SIZE}",
    },
    "timing": {
        "started_unix_ms": int("${BUILD_STARTED_MS}"),
        "finished_unix_ms": int("${BUILD_FINISHED_MS}"),
        "total_duration_ms": int("${BUILD_FINISHED_MS}") - int("${BUILD_STARTED_MS}"),
    },
    "cloud_build": json.loads(build_result_json_path.read_text()),
}
receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
PY
jq empty "$BUILD_RECEIPT_PATH" >/dev/null

BUILD_STATUS="$(jq -r '.cloud_build.status' "$BUILD_RECEIPT_PATH")"
if [[ "$BUILD_STATUS" != "SUCCESS" ]]; then
  die "Cloud Build failed: build_id=${BUILD_ID} status=${BUILD_STATUS} receipt=${BUILD_RECEIPT_PATH}"
fi

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
log "NEXUS_BUILD_RECEIPT=${BUILD_RECEIPT_PATH}"
