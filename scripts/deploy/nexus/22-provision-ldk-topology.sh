#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

run_cmd() {
  if [[ "${NEXUS_LDK_TOPOLOGY_DRY_RUN}" == "true" ]]; then
    printf '[nexus-deploy] dry-run:'
    printf ' %q' "$@"
    printf '\n'
  else
    "$@"
  fi
}

ensure_firewall_rule() {
  local name="$1"
  shift
  if [[ "${NEXUS_LDK_TOPOLOGY_DRY_RUN}" == "true" ]]; then
    run_cmd gcloud compute firewall-rules create "$name" "$@"
    return
  fi

  if firewall_rule_exists "$name"; then
    log "Updating firewall rule: ${name}"
    gcloud compute firewall-rules update "$name" "$@" >/dev/null
  else
    log "Creating firewall rule: ${name}"
    gcloud compute firewall-rules create "$name" \
      --project "$GCP_PROJECT" \
      --network "$OA_VPC" \
      "$@" >/dev/null
  fi
}

if [[ "${NEXUS_LDK_TOPOLOGY_DRY_RUN}" == "true" ]]; then
  log "Dry-running LDK topology provisioning. Set NEXUS_LDK_TOPOLOGY_DRY_RUN=false to apply."
else
  require_cmd gcloud
  ensure_gcloud_context
  ensure_services
fi

if [[ "${NEXUS_LDK_TOPOLOGY_DRY_RUN}" == "true" ]]; then
  run_cmd gcloud iam service-accounts create "$NEXUS_LDK_SERVICE_ACCOUNT_NAME" \
    --project "$GCP_PROJECT" \
    --display-name "OpenAgents Nexus LDK Server runtime"
elif ! gcloud iam service-accounts describe "$NEXUS_LDK_SERVICE_ACCOUNT_EMAIL" \
  --project "$GCP_PROJECT" >/dev/null 2>&1; then
  log "Creating service account: ${NEXUS_LDK_SERVICE_ACCOUNT_NAME}"
  gcloud iam service-accounts create "$NEXUS_LDK_SERVICE_ACCOUNT_NAME" \
    --project "$GCP_PROJECT" \
    --display-name "OpenAgents Nexus LDK Server runtime" >/dev/null
fi

for role in \
  roles/logging.logWriter \
  roles/monitoring.metricWriter; do
  log "Ensuring IAM binding: ${role}"
  run_cmd gcloud projects add-iam-policy-binding "$GCP_PROJECT" \
    --member "serviceAccount:${NEXUS_LDK_SERVICE_ACCOUNT_EMAIL}" \
    --role "$role"
done

if [[ "${NEXUS_LDK_TOPOLOGY_DRY_RUN}" == "true" ]]; then
  run_cmd gcloud compute disks create "$NEXUS_LDK_DATA_DISK" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --size "${NEXUS_LDK_DATA_DISK_SIZE_GB}GB" \
    --type "$NEXUS_LDK_DATA_DISK_TYPE"
elif ! disk_exists "$NEXUS_LDK_DATA_DISK"; then
  log "Creating LDK data disk: ${NEXUS_LDK_DATA_DISK}"
  gcloud compute disks create "$NEXUS_LDK_DATA_DISK" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --size "${NEXUS_LDK_DATA_DISK_SIZE_GB}GB" \
    --type "$NEXUS_LDK_DATA_DISK_TYPE" >/dev/null
fi

if [[ "${NEXUS_LDK_TOPOLOGY_DRY_RUN}" == "true" ]]; then
  run_cmd gcloud compute instances create "$NEXUS_LDK_VM" \
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
    --disk "name=${NEXUS_LDK_DATA_DISK},device-name=${NEXUS_LDK_DATA_DISK_DEVICE_NAME},mode=rw,auto-delete=no" \
    --tags "$NEXUS_LDK_TAG" \
    --metadata "enable-oslogin=TRUE"
elif ! instance_exists "$NEXUS_LDK_VM"; then
  log "Creating private LDK Server VM: ${NEXUS_LDK_VM}"
  gcloud compute instances create "$NEXUS_LDK_VM" \
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
    --disk "name=${NEXUS_LDK_DATA_DISK},device-name=${NEXUS_LDK_DATA_DISK_DEVICE_NAME},mode=rw,auto-delete=no" \
    --tags "$NEXUS_LDK_TAG" \
    --metadata "enable-oslogin=TRUE" >/dev/null
else
  log "LDK VM already exists: ${NEXUS_LDK_VM}"
fi

if [[ "${NEXUS_LDK_TOPOLOGY_DRY_RUN}" != "true" ]]; then
  ATTACHED_DISK_SOURCE="$(gcloud compute instances describe "$NEXUS_LDK_VM" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --format='value(disks[].source)' | grep "/disks/${NEXUS_LDK_DATA_DISK}$" || true)"
  if [[ -z "$ATTACHED_DISK_SOURCE" ]]; then
    log "Attaching missing LDK disk ${NEXUS_LDK_DATA_DISK} to ${NEXUS_LDK_VM}"
    gcloud compute instances attach-disk "$NEXUS_LDK_VM" \
      --project "$GCP_PROJECT" \
      --zone "$GCP_ZONE" \
      --disk "$NEXUS_LDK_DATA_DISK" \
      --device-name "$NEXUS_LDK_DATA_DISK_DEVICE_NAME" >/dev/null
  fi
fi

ensure_firewall_rule "$NEXUS_LDK_IAP_SSH_FIREWALL_RULE" \
  --allow tcp:22 \
  --target-tags "$NEXUS_LDK_TAG" \
  --source-ranges "35.235.240.0/20"

ensure_firewall_rule "$NEXUS_LDK_GRPC_FIREWALL_RULE" \
  --allow "tcp:${NEXUS_LDK_GRPC_PORT}" \
  --target-tags "$NEXUS_LDK_TAG" \
  --source-tags "$NEXUS_TAG"

ensure_firewall_rule "$NEXUS_BITCOIND_RPC_FIREWALL_RULE" \
  --allow "tcp:${NEXUS_BITCOIND_RPC_PORT}" \
  --target-tags "$NEXUS_BITCOIND_TAG" \
  --source-tags "$NEXUS_LDK_TAG"

if [[ "${NEXUS_LDK_ALLOW_PUBLIC_P2P}" == "true" ]]; then
  ensure_firewall_rule "$NEXUS_LDK_P2P_FIREWALL_RULE" \
    --allow "tcp:${NEXUS_LDK_P2P_PORT}" \
    --target-tags "$NEXUS_LDK_TAG" \
    --source-ranges "0.0.0.0/0"
else
  log "Skipping public Lightning P2P firewall. Set NEXUS_LDK_ALLOW_PUBLIC_P2P=true only after the node is ready to announce."
fi

log "LDK topology provisioning path complete"
log "gRPC is private: only source tag ${NEXUS_TAG} can reach target tag ${NEXUS_LDK_TAG} on tcp:${NEXUS_LDK_GRPC_PORT}."
