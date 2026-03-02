#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context
ensure_services

if ! gcloud iam service-accounts describe "$SYMPHONY_SERVICE_ACCOUNT_EMAIL" \
  --project "$GCP_PROJECT" >/dev/null 2>&1; then
  log "Creating service account: ${SYMPHONY_SERVICE_ACCOUNT_NAME}"
  gcloud iam service-accounts create "$SYMPHONY_SERVICE_ACCOUNT_NAME" \
    --project "$GCP_PROJECT" \
    --display-name "Symphony mainnet runtime" >/dev/null
fi

for role in \
  roles/logging.logWriter \
  roles/monitoring.metricWriter \
  roles/secretmanager.secretAccessor \
  roles/artifactregistry.reader; do
  log "Ensuring IAM binding: ${role}"
  gcloud projects add-iam-policy-binding "$GCP_PROJECT" \
    --member "serviceAccount:${SYMPHONY_SERVICE_ACCOUNT_EMAIL}" \
    --role "$role" >/dev/null
done

if ! disk_exists "$SYMPHONY_DATA_DISK"; then
  log "Creating disk: ${SYMPHONY_DATA_DISK}"
  gcloud compute disks create "$SYMPHONY_DATA_DISK" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --size "${SYMPHONY_DATA_DISK_SIZE_GB}GB" \
    --type "$SYMPHONY_DATA_DISK_TYPE" >/dev/null
fi

if ! instance_exists "$SYMPHONY_VM"; then
  log "Creating VM: ${SYMPHONY_VM}"
  gcloud compute instances create "$SYMPHONY_VM" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --machine-type "$SYMPHONY_MACHINE_TYPE" \
    --image-family ubuntu-2204-lts \
    --image-project ubuntu-os-cloud \
    --boot-disk-size 100GB \
    --boot-disk-type pd-ssd \
    --network-interface "subnet=${OA_SUBNET},no-address" \
    --service-account "$SYMPHONY_SERVICE_ACCOUNT_EMAIL" \
    --scopes cloud-platform \
    --disk "name=${SYMPHONY_DATA_DISK},device-name=symphony-data,mode=rw,auto-delete=no" \
    --tags "$SYMPHONY_TAG" \
    --metadata "enable-oslogin=TRUE" >/dev/null
else
  log "VM already exists: ${SYMPHONY_VM}"
fi

ATTACHED_DISK_SOURCE="$(gcloud compute instances describe "$SYMPHONY_VM" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --format='value(disks[].source)' | grep "/disks/${SYMPHONY_DATA_DISK}$" || true)"

if [[ -z "$ATTACHED_DISK_SOURCE" ]]; then
  log "Attaching missing disk ${SYMPHONY_DATA_DISK} to ${SYMPHONY_VM}"
  gcloud compute instances attach-disk "$SYMPHONY_VM" \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --disk "$SYMPHONY_DATA_DISK" \
    --device-name symphony-data >/dev/null
fi

if firewall_rule_exists "oa-allow-symphony-to-bitcoind"; then
  log "Updating firewall rule: oa-allow-symphony-to-bitcoind"
  gcloud compute firewall-rules update "oa-allow-symphony-to-bitcoind" \
    --project "$GCP_PROJECT" \
    --allow tcp:8332,tcp:8333,tcp:28332,tcp:28333 \
    --target-tags oa-bitcoind \
    --source-tags "$SYMPHONY_TAG" >/dev/null
else
  log "Creating firewall rule: oa-allow-symphony-to-bitcoind"
  gcloud compute firewall-rules create "oa-allow-symphony-to-bitcoind" \
    --project "$GCP_PROJECT" \
    --network "$OA_VPC" \
    --allow tcp:8332,tcp:8333,tcp:28332,tcp:28333 \
    --target-tags oa-bitcoind \
    --source-tags "$SYMPHONY_TAG" >/dev/null
fi

log "Baseline provisioning complete"
