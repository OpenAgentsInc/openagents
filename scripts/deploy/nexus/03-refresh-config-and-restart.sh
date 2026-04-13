#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

CURRENT_IMAGE="$(
  gcloud compute ssh "$NEXUS_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "sudo systemctl cat nexus-relay.service 2>/dev/null | awk '/^ExecStart=\\/usr\\/bin\\/docker run / { print \$NF }' | tail -n 1"
)"

[[ -n "${CURRENT_IMAGE}" ]] || die "Could not determine the current Nexus image on ${NEXUS_VM}"

log "Refreshing Nexus config on ${NEXUS_VM} using existing image ${CURRENT_IMAGE}"
DEPLOY_IMAGE="${CURRENT_IMAGE}" bash "${SCRIPT_DIR}/03-configure-and-start.sh"
