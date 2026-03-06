#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

if gcloud beta run domain-mappings describe \
  --platform=managed \
  --region "$GCP_REGION" \
  --project "$GCP_PROJECT" \
  --domain "$NEXUS_PUBLIC_HOST" >/dev/null 2>&1; then
  log "Deleting obsolete Cloud Run domain mapping for ${NEXUS_PUBLIC_HOST}"
  gcloud beta run domain-mappings delete \
    --platform=managed \
    --region "$GCP_REGION" \
    --project "$GCP_PROJECT" \
    --domain "$NEXUS_PUBLIC_HOST" \
    --quiet >/dev/null
else
  log "No Cloud Run domain mapping found for ${NEXUS_PUBLIC_HOST}"
fi

for service in openagents-nexus-relay openagents-nexus-control; do
  if gcloud run services describe "$service" \
    --platform=managed \
    --region "$GCP_REGION" \
    --project "$GCP_PROJECT" >/dev/null 2>&1; then
    log "Deleting obsolete Cloud Run service: ${service}"
    gcloud run services delete "$service" \
      --platform=managed \
      --region "$GCP_REGION" \
      --project "$GCP_PROJECT" \
      --quiet >/dev/null
  else
    log "Cloud Run service already absent: ${service}"
  fi
done

log "Legacy Cloud Run Nexus surface retired"
