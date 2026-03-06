#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context
ensure_services

if ! gcloud iam service-accounts describe "$NEXUS_SERVICE_ACCOUNT_EMAIL" \
  --project "$GCP_PROJECT" >/dev/null 2>&1; then
  log "Creating service account: ${NEXUS_SERVICE_ACCOUNT_NAME}"
  gcloud iam service-accounts create "$NEXUS_SERVICE_ACCOUNT_NAME" \
    --project "$GCP_PROJECT" \
    --display-name "OpenAgents Nexus runtime" >/dev/null
fi

for role in \
  roles/logging.logWriter \
  roles/monitoring.metricWriter \
  roles/artifactregistry.reader; do
  log "Ensuring IAM binding: ${role}"
  gcloud projects add-iam-policy-binding "$GCP_PROJECT" \
    --member "serviceAccount:${NEXUS_SERVICE_ACCOUNT_EMAIL}" \
    --role "$role" >/dev/null
done

if ! disk_exists "$NEXUS_DATA_DISK"; then
  log "Creating disk: ${NEXUS_DATA_DISK}"
  gcloud compute disks create "$NEXUS_DATA_DISK" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --size "${NEXUS_DATA_DISK_SIZE_GB}GB" \
    --type "$NEXUS_DATA_DISK_TYPE" >/dev/null
fi

if ! instance_exists "$NEXUS_VM"; then
  log "Creating VM: ${NEXUS_VM}"
  gcloud compute instances create "$NEXUS_VM" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --machine-type "$NEXUS_MACHINE_TYPE" \
    --image-family ubuntu-2204-lts \
    --image-project ubuntu-os-cloud \
    --boot-disk-size 100GB \
    --boot-disk-type pd-ssd \
    --network-interface "subnet=${OA_SUBNET},no-address" \
    --service-account "$NEXUS_SERVICE_ACCOUNT_EMAIL" \
    --scopes cloud-platform \
    --disk "name=${NEXUS_DATA_DISK},device-name=${NEXUS_DATA_DISK_DEVICE_NAME},mode=rw,auto-delete=no" \
    --tags "$NEXUS_TAG" \
    --metadata "enable-oslogin=TRUE" >/dev/null
else
  log "VM already exists: ${NEXUS_VM}"
fi

ATTACHED_DISK_SOURCE="$(gcloud compute instances describe "$NEXUS_VM" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --format='value(disks[].source)' | grep "/disks/${NEXUS_DATA_DISK}$" || true)"

if [[ -z "$ATTACHED_DISK_SOURCE" ]]; then
  log "Attaching missing disk ${NEXUS_DATA_DISK} to ${NEXUS_VM}"
  gcloud compute instances attach-disk "$NEXUS_VM" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --disk "$NEXUS_DATA_DISK" \
    --device-name "$NEXUS_DATA_DISK_DEVICE_NAME" >/dev/null
fi

if firewall_rule_exists "oa-allow-nexus-iap-ssh"; then
  log "Updating firewall rule: oa-allow-nexus-iap-ssh"
  gcloud compute firewall-rules update "oa-allow-nexus-iap-ssh" \
    --project "$GCP_PROJECT" \
    --allow tcp:22 \
    --target-tags "$NEXUS_TAG" \
    --source-ranges "35.235.240.0/20" >/dev/null
else
  log "Creating firewall rule: oa-allow-nexus-iap-ssh"
  gcloud compute firewall-rules create "oa-allow-nexus-iap-ssh" \
    --project "$GCP_PROJECT" \
    --network "$OA_VPC" \
    --allow tcp:22 \
    --target-tags "$NEXUS_TAG" \
    --source-ranges "35.235.240.0/20" >/dev/null
fi

log "Baseline provisioning complete"
log "This baseline remains private-by-default; public DNS/TLS cutover is handled in later Nexus migration issues."
