#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud
require_cmd jq

ensure_gcloud_context

DEPLOY_IMAGE="${DEPLOY_IMAGE:-${NEXUS_IMAGE}}"
REPORT_DIR="${ROOT_DIR}/docs/reports/nexus"
STAMP="$(date -u +%Y%m%d-%H%M%S)"
RECEIPT_PATH="${REPORT_DIR}/${STAMP}-deploy-receipt.json"

mkdir -p "$REPORT_DIR"

if ! instance_exists "$NEXUS_VM"; then
  die "VM does not exist: ${NEXUS_VM}. Run 02-provision-baseline.sh first."
fi

INSTANCE_STATUS="$(gcloud compute instances describe "$NEXUS_VM" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --format='value(status)')"

HEALTH_RAW="$(gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "curl -fsS http://127.0.0.1:8080/healthz")"

STATS_RAW="$(gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "curl -fsS http://127.0.0.1:8080/api/stats")"

SERVICE_STATUS_RAW="$(gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "systemctl is-active nexus-relay")"

DATA_DIR_STATUS_RAW="$(gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "mount | grep '${NEXUS_DATA_DIR}' || true")"

jq -n \
  --arg generated_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg vm "$NEXUS_VM" \
  --arg instance_status "$INSTANCE_STATUS" \
  --arg service_status "${SERVICE_STATUS_RAW//$'\n'/}" \
  --arg image "$DEPLOY_IMAGE" \
  --arg health_raw "$HEALTH_RAW" \
  --arg stats_raw "$STATS_RAW" \
  --arg data_mount "$DATA_DIR_STATUS_RAW" \
  '{
    generated_at: $generated_at,
    vm: $vm,
    instance_status: $instance_status,
    service_status: $service_status,
    image: $image,
    health: ($health_raw | fromjson),
    stats: ($stats_raw | fromjson),
    data_mount: $data_mount
  }' >"$RECEIPT_PATH"

log "Wrote deploy receipt: ${RECEIPT_PATH}"
