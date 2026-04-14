#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

if ! instance_exists "$NEXUS_VM"; then
  die "VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

: "${NEXUS_TREASURY_RECOVERY_REPORT_PATH:?Set NEXUS_TREASURY_RECOVERY_REPORT_PATH to the validated recovery report path on the VM data disk}"

DEPLOY_IMAGE="${DEPLOY_IMAGE:-${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${NEXUS_ARTIFACT_REPO}/${NEXUS_IMAGE_NAME}:latest}"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "set -euo pipefail; \
    sudo systemctl stop nexus-relay; \
    ACCESS_TOKEN=\$(curl -fsS -H 'Metadata-Flavor: Google' 'http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token' | jq -r '.access_token'); \
    AR_HOST=\$(echo '$DEPLOY_IMAGE' | cut -d'/' -f1); \
    echo \"\$ACCESS_TOKEN\" | sudo docker login -u oauth2accesstoken --password-stdin \"https://\${AR_HOST}\" >/dev/null; \
    sudo docker pull '$DEPLOY_IMAGE' >/dev/null; \
    sudo docker run --rm \
      --network host \
      --env-file /etc/nexus-relay/nexus-relay.env \
      -v '${NEXUS_DATA_DIR}:${NEXUS_DATA_DIR}' \
      -v '/etc/nexus-relay:/etc/nexus-relay:ro' \
      '$DEPLOY_IMAGE' \
      treasury recovery-cutover --report-path '${NEXUS_TREASURY_RECOVERY_REPORT_PATH}' --json; \
    sudo systemctl start nexus-relay; \
    systemctl is-active nexus-relay >/dev/null; \
    curl -fsS http://127.0.0.1:8080/v1/treasury/status >/dev/null"

log "Treasury wallet recovery cutover completed on ${NEXUS_VM}"
