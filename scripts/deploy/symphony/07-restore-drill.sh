#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

if ! disk_exists "$SYMPHONY_DATA_DISK"; then
  die "Symphony data disk not found: ${SYMPHONY_DATA_DISK}"
fi

STAMP="$(date -u +%Y%m%d-%H%M%S)"
SNAPSHOT_NAME="symphony-data-drill-${STAMP}"
RESTORE_DISK_NAME="symphony-data-restore-${STAMP}"
REPORT_DIR="${REPORT_DIR:-$ROOT_DIR/docs/reports/symphony}"
REPORT_PATH="${REPORT_DIR}/${STAMP}-restore-drill.md"
mkdir -p "$REPORT_DIR"

log "Creating snapshot: ${SNAPSHOT_NAME}"
gcloud compute disks snapshot "$SYMPHONY_DATA_DISK" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --snapshot-names "$SNAPSHOT_NAME" >/dev/null

log "Creating restore disk from snapshot: ${RESTORE_DISK_NAME}"
gcloud compute disks create "$RESTORE_DISK_NAME" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --source-snapshot "$SNAPSHOT_NAME" \
  --type "$SYMPHONY_DATA_DISK_TYPE" >/dev/null

RESTORE_DISK_SIZE="$(gcloud compute disks describe "$RESTORE_DISK_NAME" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --format='value(sizeGb)')"

cat >"$REPORT_PATH" <<REPORT
# Symphony Restore Drill Report

- Timestamp (UTC): ${STAMP}
- Project: ${GCP_PROJECT}
- Region: ${GCP_REGION}
- Zone: ${GCP_ZONE}
- Source disk: ${SYMPHONY_DATA_DISK}
- Snapshot created: ${SNAPSHOT_NAME}
- Restore disk created: ${RESTORE_DISK_NAME}
- Restore disk size (GB): ${RESTORE_DISK_SIZE}

## Commands executed

\`gcloud compute disks snapshot ${SYMPHONY_DATA_DISK} --zone ${GCP_ZONE} --snapshot-names ${SNAPSHOT_NAME}\`

\`gcloud compute disks create ${RESTORE_DISK_NAME} --zone ${GCP_ZONE} --source-snapshot ${SNAPSHOT_NAME}\`

## Outcome

- Snapshot creation: success
- Restore disk creation: success
- Next step for full validation: attach restore disk to a recovery VM and run Symphony read checks before promoting.
REPORT

if [[ "${DELETE_RESTORE_DISK:-1}" == "1" ]]; then
  log "Deleting temporary restore disk: ${RESTORE_DISK_NAME}"
  gcloud compute disks delete "$RESTORE_DISK_NAME" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --quiet >/dev/null
fi

log "Restore drill report written: ${REPORT_PATH}"
cat "$REPORT_PATH"
