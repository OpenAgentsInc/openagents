#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

BUILD_CONTEXT_SCRIPT="${SCRIPT_DIR}/stage-build-context.sh"
REMOTE_BUILD_HELPER="${SCRIPT_DIR}/remote-build-nexus-binary.sh"
REPORT_DIR="${ROOT_DIR}/docs/reports/nexus"
NEXUS_BUILDER_CLEAR_CACHES="${NEXUS_BUILDER_CLEAR_CACHES:-false}"

require_cmd git
require_cmd gcloud
require_cmd jq
require_cmd tar

[[ -f "$BUILD_CONTEXT_SCRIPT" ]] || die "Missing build context helper: ${BUILD_CONTEXT_SCRIPT}"
[[ -f "$REMOTE_BUILD_HELPER" ]] || die "Missing remote build helper: ${REMOTE_BUILD_HELPER}"

ensure_gcloud_context

if ! instance_exists "$NEXUS_BUILDER_VM"; then
  die "Warm builder VM does not exist: ${NEXUS_BUILDER_VM}. Run 11-provision-warm-builder.sh first."
fi

GIT_SHA="$(git -C "$ROOT_DIR" rev-parse HEAD)"
GIT_SHORT_SHA="$(git -C "$ROOT_DIR" rev-parse --short=12 HEAD)"
GIT_REF="$(git -C "$ROOT_DIR" symbolic-ref --quiet --short HEAD || echo detached)"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
LOCAL_RECEIPT_PATH="${REPORT_DIR}/${STAMP}-warm-builder-build-${GIT_SHORT_SHA}.json"

TMP_CONTEXT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/openagents-nexus-build-context.XXXXXX")"
TMP_CONTEXT_TARBALL="$(mktemp "${TMPDIR:-/tmp}/openagents-nexus-build-context.XXXXXX.tar.gz")"
TMP_REMOTE_BUILD_HELPER="$(mktemp "${TMPDIR:-/tmp}/remote-build-nexus-binary.XXXXXX")"
trap 'rm -rf "$TMP_CONTEXT_DIR" "$TMP_CONTEXT_TARBALL" "$TMP_REMOTE_BUILD_HELPER"' EXIT

bash "$BUILD_CONTEXT_SCRIPT" "$TMP_CONTEXT_DIR" >/dev/null
cp "$REMOTE_BUILD_HELPER" "$TMP_REMOTE_BUILD_HELPER"
chmod 755 "$TMP_REMOTE_BUILD_HELPER"

if command -v xattr >/dev/null 2>&1; then
  xattr -rc "$TMP_CONTEXT_DIR" 2>/dev/null || true
fi

TAR_ARGS=(-C "$TMP_CONTEXT_DIR" -czf "$TMP_CONTEXT_TARBALL")
if tar --help 2>/dev/null | grep -q -- '--no-mac-metadata'; then
  TAR_ARGS+=(--no-mac-metadata)
fi
if tar --help 2>/dev/null | grep -q -- '--no-xattrs'; then
  TAR_ARGS+=(--no-xattrs)
fi
COPYFILE_DISABLE=1 COPY_EXTENDED_ATTRIBUTES_DISABLE=1 tar "${TAR_ARGS[@]}" .

BUILD_CONTEXT_FILE_COUNT="$(find "$TMP_CONTEXT_DIR" -type f | wc -l | tr -d '[:space:]')"
BUILD_CONTEXT_SIZE="$(du -sh "$TMP_CONTEXT_DIR" | awk '{print $1}')"

log "Submitting Nexus binary build to ${NEXUS_BUILDER_VM}"
log "context_files=${BUILD_CONTEXT_FILE_COUNT} context_size=${BUILD_CONTEXT_SIZE} profile=${NEXUS_BUILDER_BUILD_PROFILE} clear_caches=${NEXUS_BUILDER_CLEAR_CACHES}"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_CONTEXT_TARBALL" "${NEXUS_BUILDER_VM}:/tmp/openagents-nexus-build-context.tar.gz" >/dev/null

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_BUILD_HELPER" "${NEXUS_BUILDER_VM}:/tmp/remote-build-nexus-binary.sh" >/dev/null

REMOTE_METADATA_JSON="$(
  gcloud compute ssh "$NEXUS_BUILDER_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "chmod 644 /tmp/openagents-nexus-build-context.tar.gz && chmod 755 /tmp/remote-build-nexus-binary.sh && NEXUS_BUILDER_CACHE_MOUNT_POINT='${NEXUS_BUILDER_CACHE_MOUNT_POINT}' NEXUS_BUILDER_USER='${NEXUS_BUILDER_USER}' /tmp/remote-build-nexus-binary.sh '/tmp/openagents-nexus-build-context.tar.gz' '${GIT_SHA}' '${GIT_SHORT_SHA}' '${GIT_REF}' '${NEXUS_BUILDER_BUILD_PROFILE}' '${NEXUS_BUILDER_CLEAR_CACHES}'"
)"

mkdir -p "$REPORT_DIR"
printf '%s\n' "$REMOTE_METADATA_JSON" >"$LOCAL_RECEIPT_PATH"
jq empty "$LOCAL_RECEIPT_PATH"

log "Warm builder artifact ready"
log "receipt=${LOCAL_RECEIPT_PATH}"
log "artifact_root=$(jq -r '.artifact_root' "$LOCAL_RECEIPT_PATH")"
log "binary_sha256=$(jq -r '.binary_sha256' "$LOCAL_RECEIPT_PATH")"
