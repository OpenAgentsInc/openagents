#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

run_cmd() {
  if [[ "${NEXUS_LDK_BACKUP_DRY_RUN}" == "true" ]]; then
    printf '[nexus-deploy] dry-run:'
    printf ' %q' "$@"
    printf '\n'
  else
    "$@"
  fi
}

if [[ "${NEXUS_LDK_BACKUP_DRY_RUN}" == "true" ]]; then
  log "Dry-running LDK Server backup. Set NEXUS_LDK_BACKUP_DRY_RUN=false to apply."
else
  require_cmd gcloud
  require_cmd gsutil
  ensure_gcloud_context
  instance_exists "$NEXUS_LDK_VM" || die "LDK VM does not exist: ${NEXUS_LDK_VM}"
fi

STAMP="$(date -u +%Y%m%d-%H%M%S)"
SNAPSHOT_NAME="${NEXUS_LDK_DATA_DISK}-${STAMP}"
REMOTE_DIR="/tmp/ldk-backup-${STAMP}"
REMOTE_ARCHIVE="/tmp/ldk-backup-${STAMP}.tar.gz"
GCS_ARCHIVE="${NEXUS_LDK_BACKUP_BUCKET%/}/${NEXUS_LDK_BACKUP_PREFIX}/${STAMP}/ldk-server-state.tar.gz"

run_cmd gcloud compute snapshots create "$SNAPSHOT_NAME" \
  --project "$GCP_PROJECT" \
  --source-disk "$NEXUS_LDK_DATA_DISK" \
  --source-disk-zone "$GCP_ZONE" \
  --description "OpenAgents Nexus LDK Server state snapshot ${STAMP}"

run_cmd gcloud compute ssh "$NEXUS_LDK_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "sudo env \
    NEXUS_LDK_STORAGE_DIR='${NEXUS_LDK_STORAGE_DIR}' \
    NEXUS_LDK_CONFIG_DIR='${NEXUS_LDK_CONFIG_DIR}' \
    NEXUS_LDK_NETWORK='${NEXUS_LDK_NETWORK}' \
    REMOTE_DIR='${REMOTE_DIR}' \
    REMOTE_ARCHIVE='${REMOTE_ARCHIVE}' \
    bash -s" <<'REMOTE'
set -euo pipefail
rm -rf "$REMOTE_DIR" "$REMOTE_ARCHIVE"
mkdir -p "$REMOTE_DIR"
systemctl is-active --quiet ldk-server.service
cp -a "$NEXUS_LDK_STORAGE_DIR/keys_seed" "$REMOTE_DIR/keys_seed"
cp -a "$NEXUS_LDK_STORAGE_DIR/tls.crt" "$REMOTE_DIR/tls.crt"
if [[ -f "$NEXUS_LDK_STORAGE_DIR/tls.key" ]]; then
  cp -a "$NEXUS_LDK_STORAGE_DIR/tls.key" "$REMOTE_DIR/tls.key"
fi
mkdir -p "$REMOTE_DIR/${NEXUS_LDK_NETWORK}"
cp -a "$NEXUS_LDK_STORAGE_DIR/${NEXUS_LDK_NETWORK}/api_key" "$REMOTE_DIR/${NEXUS_LDK_NETWORK}/api_key"
cp -a "$NEXUS_LDK_STORAGE_DIR/${NEXUS_LDK_NETWORK}/ldk_node_data.sqlite" "$REMOTE_DIR/${NEXUS_LDK_NETWORK}/ldk_node_data.sqlite"
if [[ -f "$NEXUS_LDK_STORAGE_DIR/${NEXUS_LDK_NETWORK}/ldk_server_data.sqlite" ]]; then
  cp -a "$NEXUS_LDK_STORAGE_DIR/${NEXUS_LDK_NETWORK}/ldk_server_data.sqlite" "$REMOTE_DIR/${NEXUS_LDK_NETWORK}/ldk_server_data.sqlite"
fi
cp -a "$NEXUS_LDK_CONFIG_DIR" "$REMOTE_DIR/config"
python3 - <<PY >"$REMOTE_DIR/metadata.json"
import json, time
print(json.dumps({
  "generated_at_unix_ms": int(time.time() * 1000),
  "storage_dir": "${NEXUS_LDK_STORAGE_DIR}",
  "network": "${NEXUS_LDK_NETWORK}",
  "contains_secret_material": True,
}, indent=2))
PY
tar -C "$REMOTE_DIR" -czf "$REMOTE_ARCHIVE" .
chmod 0600 "$REMOTE_ARCHIVE"
REMOTE

run_cmd gcloud compute scp \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "${NEXUS_LDK_VM}:${REMOTE_ARCHIVE}" /tmp/

run_cmd gsutil cp "$REMOTE_ARCHIVE" "$GCS_ARCHIVE"

run_cmd gcloud compute ssh "$NEXUS_LDK_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "rm -rf '${REMOTE_DIR}' '${REMOTE_ARCHIVE}'"

log "LDK backup path complete"
log "Snapshot: ${SNAPSHOT_NAME}"
log "Archive: ${GCS_ARCHIVE}"
