#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

if ! instance_exists "$NEXUS_VM"; then
  die "VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

: "${NEXUS_BACKUP_ARCHIVE:?Set NEXUS_BACKUP_ARCHIVE to the local backup archive path}"
[[ -f "$NEXUS_BACKUP_ARCHIVE" ]] || die "Backup archive does not exist: ${NEXUS_BACKUP_ARCHIVE}"

STAMP="$(date -u +%Y%m%d-%H%M%S)"
REMOTE_ARCHIVE="/tmp/nexus-restore-${STAMP}.tar.gz"
REMOTE_DIR="/tmp/nexus-restore-${STAMP}"
PRE_RESTORE_DIR="${NEXUS_DATA_DIR}/pre-restore-${STAMP}"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$NEXUS_BACKUP_ARCHIVE" "${NEXUS_VM}:${REMOTE_ARCHIVE}"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "set -euo pipefail; rm -rf '${REMOTE_DIR}'; mkdir -p '${REMOTE_DIR}'; tar -C '${REMOTE_DIR}' -xzf '${REMOTE_ARCHIVE}'; sudo systemctl stop nexus-relay; sudo mkdir -p '${PRE_RESTORE_DIR}'; if [[ -f '${NEXUS_DATA_DIR}/nostr.db' ]]; then sudo mv '${NEXUS_DATA_DIR}/nostr.db' '${PRE_RESTORE_DIR}/nostr.db'; fi; if [[ -f '${NEXUS_DATA_DIR}/nostr.db-shm' ]]; then sudo mv '${NEXUS_DATA_DIR}/nostr.db-shm' '${PRE_RESTORE_DIR}/nostr.db-shm'; fi; if [[ -f '${NEXUS_DATA_DIR}/nostr.db-wal' ]]; then sudo mv '${NEXUS_DATA_DIR}/nostr.db-wal' '${PRE_RESTORE_DIR}/nostr.db-wal'; fi; if [[ -f '${NEXUS_RECEIPT_LOG_PATH}' ]]; then sudo mv '${NEXUS_RECEIPT_LOG_PATH}' '${PRE_RESTORE_DIR}/nexus-control-receipts.jsonl'; fi; sudo cp '${REMOTE_DIR}/nostr.db' '${NEXUS_DATA_DIR}/nostr.db'; sudo chown 60000:60000 '${NEXUS_DATA_DIR}/nostr.db'; if [[ -f '${REMOTE_DIR}/nexus-control-receipts.jsonl' ]]; then sudo cp '${REMOTE_DIR}/nexus-control-receipts.jsonl' '${NEXUS_RECEIPT_LOG_PATH}'; sudo chown 60000:999 '${NEXUS_RECEIPT_LOG_PATH}' || sudo chown 60000:60000 '${NEXUS_RECEIPT_LOG_PATH}'; fi; sudo rm -f '${NEXUS_DATA_DIR}/nostr.db-shm' '${NEXUS_DATA_DIR}/nostr.db-wal'; sudo systemctl start nexus-relay; systemctl is-active nexus-relay; rm -rf '${REMOTE_DIR}' '${REMOTE_ARCHIVE}'"

log "Restore completed on ${NEXUS_VM} from ${NEXUS_BACKUP_ARCHIVE}"
