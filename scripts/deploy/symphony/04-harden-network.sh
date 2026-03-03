#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

SYMPHONY_API_ALLOWED_CIDRS="${SYMPHONY_API_ALLOWED_CIDRS:-10.42.0.0/24,10.42.8.0/28}"

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

if firewall_rule_exists "oa-allow-symphony-api"; then
  log "Updating firewall rule: oa-allow-symphony-api"
  gcloud compute firewall-rules update "oa-allow-symphony-api" \
    --project "$GCP_PROJECT" \
    --allow tcp:8080 \
    --target-tags "$SYMPHONY_TAG" \
    --source-ranges "$SYMPHONY_API_ALLOWED_CIDRS" >/dev/null
else
  log "Creating firewall rule: oa-allow-symphony-api"
  gcloud compute firewall-rules create "oa-allow-symphony-api" \
    --project "$GCP_PROJECT" \
    --network "$OA_VPC" \
    --allow tcp:8080 \
    --target-tags "$SYMPHONY_TAG" \
    --source-ranges "$SYMPHONY_API_ALLOWED_CIDRS" >/dev/null
fi

EXTERNAL_IP="$(gcloud compute instances describe "$SYMPHONY_VM" \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --format='get(networkInterfaces[0].accessConfigs[0].natIP)' || true)"

if [[ -n "$EXTERNAL_IP" ]]; then
  log "WARNING: ${SYMPHONY_VM} has external IP ${EXTERNAL_IP}. Remove it to keep API private."
else
  log "Verified ${SYMPHONY_VM} has no external IP (private-only ingress posture)."
fi

log "Hardening apply complete"
