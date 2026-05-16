#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

run_cmd() {
  if [[ "${NEXUS_LDK_RESTORE_DRY_RUN}" == "true" ]]; then
    printf '[nexus-deploy] dry-run:'
    printf ' %q' "$@"
    printf '\n'
  else
    "$@"
  fi
}

if [[ "${NEXUS_LDK_RESTORE_DRY_RUN}" == "true" ]]; then
  log "Dry-running LDK Server restore drill. Set NEXUS_LDK_RESTORE_DRY_RUN=false to apply."
else
  require_cmd gcloud
  ensure_gcloud_context
  : "${NEXUS_LDK_RESTORE_SNAPSHOT:?Set NEXUS_LDK_RESTORE_SNAPSHOT to the snapshot created by 25-backup-ldk-server-state.sh}"
fi

run_cmd gcloud compute disks create "$NEXUS_LDK_RESTORE_DRILL_DISK" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --source-snapshot "$NEXUS_LDK_RESTORE_SNAPSHOT" \
  --type "$NEXUS_LDK_DATA_DISK_TYPE"

run_cmd gcloud compute instances create "$NEXUS_LDK_RESTORE_DRILL_VM" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --machine-type "$NEXUS_LDK_MACHINE_TYPE" \
  --image-family "$NEXUS_LDK_IMAGE_FAMILY" \
  --image-project "$NEXUS_LDK_IMAGE_PROJECT" \
  --boot-disk-size "${NEXUS_LDK_BOOT_DISK_SIZE_GB}GB" \
  --boot-disk-type pd-ssd \
  --network-interface "subnet=${OA_SUBNET},no-address" \
  --service-account "$NEXUS_LDK_SERVICE_ACCOUNT_EMAIL" \
  --scopes cloud-platform \
  --disk "name=${NEXUS_LDK_RESTORE_DRILL_DISK},device-name=${NEXUS_LDK_DATA_DISK_DEVICE_NAME},mode=ro,auto-delete=no" \
  --tags "$NEXUS_LDK_TAG" \
  --metadata "enable-oslogin=TRUE"

run_cmd gcloud compute ssh "$NEXUS_LDK_RESTORE_DRILL_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "sudo env \
    NEXUS_LDK_DATA_DISK_DEVICE_NAME='${NEXUS_LDK_DATA_DISK_DEVICE_NAME}' \
    NEXUS_LDK_STORAGE_DIR='${NEXUS_LDK_STORAGE_DIR}' \
    NEXUS_LDK_NETWORK='${NEXUS_LDK_NETWORK}' \
    bash -s" <<'REMOTE'
set -euo pipefail
DEVICE_PATH="/dev/disk/by-id/google-${NEXUS_LDK_DATA_DISK_DEVICE_NAME}"
test -e "$DEVICE_PATH"
mkdir -p "$NEXUS_LDK_STORAGE_DIR"
mount -o ro "$DEVICE_PATH" "$NEXUS_LDK_STORAGE_DIR"
test -r "$NEXUS_LDK_STORAGE_DIR/keys_seed"
test -r "$NEXUS_LDK_STORAGE_DIR/tls.crt"
test -r "$NEXUS_LDK_STORAGE_DIR/${NEXUS_LDK_NETWORK}/api_key"
test -r "$NEXUS_LDK_STORAGE_DIR/${NEXUS_LDK_NETWORK}/ldk_node_data.sqlite"
python3 - <<PY
import json, os, time
storage = os.environ["NEXUS_LDK_STORAGE_DIR"]
network = os.environ["NEXUS_LDK_NETWORK"]
paths = [
    "keys_seed",
    "tls.crt",
    f"{network}/api_key",
    f"{network}/ldk_node_data.sqlite",
]
print(json.dumps({
    "generated_at_unix_ms": int(time.time() * 1000),
    "restore_drill": "read_only_mount",
    "verified_paths": paths,
    "storage_dir": storage,
}, indent=2))
PY
umount "$NEXUS_LDK_STORAGE_DIR"
REMOTE

log "LDK restore drill completed on ${NEXUS_LDK_RESTORE_DRILL_VM}"
log "Manual cleanup after review:"
log "  gcloud compute instances delete ${NEXUS_LDK_RESTORE_DRILL_VM} --zone ${GCP_ZONE}"
log "  gcloud compute disks delete ${NEXUS_LDK_RESTORE_DRILL_DISK} --zone ${GCP_ZONE}"
