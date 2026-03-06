#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

if ! instance_exists "$NEXUS_VM"; then
  die "VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

LOCAL_BACKUP_DIR="${NEXUS_BACKUP_LOCAL_DIR:-${HOME}/backups/nexus}"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
REMOTE_DIR="/tmp/nexus-backup-${STAMP}"
REMOTE_ARCHIVE="/tmp/nexus-backup-${STAMP}.tar.gz"
LOCAL_ARCHIVE="${LOCAL_BACKUP_DIR}/nexus-backup-${NEXUS_VM}-${STAMP}.tar.gz"

mkdir -p "$LOCAL_BACKUP_DIR"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "set -euo pipefail; rm -rf '${REMOTE_DIR}' '${REMOTE_ARCHIVE}'; mkdir -p '${REMOTE_DIR}'; sqlite3 '${NEXUS_DATA_DIR}/nostr.db' \".backup '${REMOTE_DIR}/nostr.db'\"; if [[ -f '${NEXUS_RECEIPT_LOG_PATH}' ]]; then cp '${NEXUS_RECEIPT_LOG_PATH}' '${REMOTE_DIR}/nexus-control-receipts.jsonl'; fi; python3 -c \"import json,time; print(json.dumps({'generated_at_unix_ms': int(time.time() * 1000), 'vm': '${NEXUS_VM}', 'data_dir': '${NEXUS_DATA_DIR}', 'receipt_log_path': '${NEXUS_RECEIPT_LOG_PATH}'}, indent=2))\" > '${REMOTE_DIR}/metadata.json'; cd '${REMOTE_DIR}' && tar -czf '${REMOTE_ARCHIVE}' ."

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "${NEXUS_VM}:${REMOTE_ARCHIVE}" "$LOCAL_ARCHIVE"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "rm -rf '${REMOTE_DIR}' '${REMOTE_ARCHIVE}'"

log "Wrote backup archive: ${LOCAL_ARCHIVE}"
