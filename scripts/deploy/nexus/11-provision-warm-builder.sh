#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

BOOTSTRAP_HELPER="${SCRIPT_DIR}/remote-bootstrap-warm-builder.sh"

require_cmd gcloud

[[ -f "$BOOTSTRAP_HELPER" ]] || die "Missing builder bootstrap helper: ${BOOTSTRAP_HELPER}"

ensure_gcloud_context
ensure_services

wait_for_builder_ssh() {
  local attempt
  for attempt in $(seq 1 30); do
    if gcloud compute ssh "$NEXUS_BUILDER_VM" \
      --tunnel-through-iap \
      --project "$GCP_PROJECT" \
      --zone "$GCP_ZONE" \
      --command "true" >/dev/null 2>&1; then
      return 0
    fi
    log "Waiting for ${NEXUS_BUILDER_VM} SSH readiness via IAP (${attempt}/30)"
    sleep 10
  done
  return 1
}

if ! disk_exists "$NEXUS_BUILDER_CACHE_DISK"; then
  log "Creating warm builder cache disk: ${NEXUS_BUILDER_CACHE_DISK}"
  gcloud compute disks create "$NEXUS_BUILDER_CACHE_DISK" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --size "${NEXUS_BUILDER_CACHE_DISK_SIZE_GB}GB" \
    --type "${NEXUS_BUILDER_CACHE_DISK_TYPE}" >/dev/null
fi

if ! firewall_rule_exists "$NEXUS_BUILDER_IAP_SSH_FIREWALL_RULE"; then
  log "Creating IAP SSH firewall rule for ${NEXUS_BUILDER_TAG}"
  gcloud compute firewall-rules create "$NEXUS_BUILDER_IAP_SSH_FIREWALL_RULE" \
    --project "$GCP_PROJECT" \
    --network "$OA_VPC" \
    --direction INGRESS \
    --action ALLOW \
    --rules tcp:22 \
    --source-ranges 35.235.240.0/20 \
    --target-tags "$NEXUS_BUILDER_TAG" >/dev/null
fi

if ! instance_exists "$NEXUS_BUILDER_VM"; then
  log "Creating warm builder VM: ${NEXUS_BUILDER_VM}"
  gcloud compute instances create "$NEXUS_BUILDER_VM" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --machine-type "$NEXUS_BUILDER_MACHINE_TYPE" \
    --subnet "$OA_SUBNET" \
    --no-address \
    --tags "$NEXUS_BUILDER_TAG" \
    --service-account "$NEXUS_BUILDER_SERVICE_ACCOUNT_EMAIL" \
    --scopes "https://www.googleapis.com/auth/cloud-platform" \
    --image-family "$NEXUS_BUILDER_IMAGE_FAMILY" \
    --image-project "$NEXUS_BUILDER_IMAGE_PROJECT" \
    --boot-disk-size "${NEXUS_BUILDER_BOOT_DISK_SIZE_GB}GB" \
    --boot-disk-type "$NEXUS_BUILDER_BOOT_DISK_TYPE" \
    --disk "name=${NEXUS_BUILDER_CACHE_DISK},device-name=${NEXUS_BUILDER_CACHE_DISK_DEVICE_NAME},mode=rw,boot=no,auto-delete=no" >/dev/null
else
  INSTANCE_STATUS="$(gcloud compute instances describe "$NEXUS_BUILDER_VM" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --format='value(status)')"
  if [[ "$INSTANCE_STATUS" != "RUNNING" ]]; then
    log "Starting existing warm builder VM: ${NEXUS_BUILDER_VM}"
    gcloud compute instances start "$NEXUS_BUILDER_VM" \
      --project "$GCP_PROJECT" \
      --zone "$GCP_ZONE" >/dev/null
  fi

  ATTACHED_DEVICE_NAMES="$(gcloud compute instances describe "$NEXUS_BUILDER_VM" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --format='value(disks[].deviceName)')"
  if ! grep -Eq "(^|[;\n])${NEXUS_BUILDER_CACHE_DISK_DEVICE_NAME}($|[;\n])" <<<"$ATTACHED_DEVICE_NAMES"; then
    log "Attaching warm builder cache disk to ${NEXUS_BUILDER_VM}"
    gcloud compute instances attach-disk "$NEXUS_BUILDER_VM" \
      --project "$GCP_PROJECT" \
      --zone "$GCP_ZONE" \
      --disk "$NEXUS_BUILDER_CACHE_DISK" \
      --device-name "$NEXUS_BUILDER_CACHE_DISK_DEVICE_NAME" >/dev/null
  fi
fi

wait_for_builder_ssh || die "Warm builder VM did not become SSH-ready: ${NEXUS_BUILDER_VM}"

TMP_REMOTE_BOOTSTRAP="$(mktemp)"
trap 'rm -f "$TMP_REMOTE_BOOTSTRAP"' EXIT
cp "$BOOTSTRAP_HELPER" "$TMP_REMOTE_BOOTSTRAP"
chmod 755 "$TMP_REMOTE_BOOTSTRAP"

gcloud compute scp --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  "$TMP_REMOTE_BOOTSTRAP" "${NEXUS_BUILDER_VM}:/tmp/remote-bootstrap-warm-builder.sh" >/dev/null

gcloud compute ssh "$NEXUS_BUILDER_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "chmod +x /tmp/remote-bootstrap-warm-builder.sh && /tmp/remote-bootstrap-warm-builder.sh '${NEXUS_BUILDER_CACHE_DISK_DEVICE_NAME}' '${NEXUS_BUILDER_CACHE_MOUNT_POINT}' '${NEXUS_BUILDER_USER}' '${NEXUS_BUILDER_RUST_TOOLCHAIN}' '${NEXUS_BUILDER_SCCACHE_VERSION}'"

log "Warm builder ready on ${NEXUS_BUILDER_VM}"
log "cache_mount=${NEXUS_BUILDER_CACHE_MOUNT_POINT}"
log "builder_user=${NEXUS_BUILDER_USER}"
