#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
source "${SCRIPT_DIR}/common.sh"

if [[ "${NEXUS_LDK_REMOTE_SMOKE}" != "true" ]]; then
  log "Running local LDK client-boundary smoke. Set NEXUS_LDK_REMOTE_SMOKE=true for hosted VM read-only smoke."
  (
    cd "$ROOT_DIR"
    cargo test -p nexus-control ldk_server -- --nocapture
  )
  log "Local LDK client-boundary smoke completed"
  exit 0
fi

require_cmd gcloud
ensure_gcloud_context
instance_exists "$NEXUS_LDK_VM" || die "LDK VM does not exist: ${NEXUS_LDK_VM}"

gcloud compute ssh "$NEXUS_LDK_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "sudo env \
    NEXUS_LDK_STORAGE_DIR='${NEXUS_LDK_STORAGE_DIR}' \
    NEXUS_LDK_NETWORK='${NEXUS_LDK_NETWORK}' \
    NEXUS_LDK_GRPC_PORT='${NEXUS_LDK_GRPC_PORT}' \
    bash -s" <<'REMOTE'
set -euo pipefail

systemctl is-active --quiet ldk-server.service
test -r "${NEXUS_LDK_STORAGE_DIR}/tls.crt"
test -r "${NEXUS_LDK_STORAGE_DIR}/${NEXUS_LDK_NETWORK}/api_key"
test -r "${NEXUS_LDK_STORAGE_DIR}/${NEXUS_LDK_NETWORK}/ldk_node_data.sqlite"

API_KEY_HEX="$(xxd -p -c 64 "${NEXUS_LDK_STORAGE_DIR}/${NEXUS_LDK_NETWORK}/api_key")"
ldk-server-cli \
  --base-url "localhost:${NEXUS_LDK_GRPC_PORT}" \
  --api-key "$API_KEY_HEX" \
  --tls-cert "${NEXUS_LDK_STORAGE_DIR}/tls.crt" \
  get-node-info >/tmp/ldk-node-info.json

ldk-server-cli \
  --base-url "localhost:${NEXUS_LDK_GRPC_PORT}" \
  --api-key "$API_KEY_HEX" \
  --tls-cert "${NEXUS_LDK_STORAGE_DIR}/tls.crt" \
  get-balances >/tmp/ldk-balances.json

curl -fsSk --max-time 10 "https://localhost:${NEXUS_LDK_GRPC_PORT}/metrics" |
  grep -E 'ldk|lightning|payment|balance|channel' >/tmp/ldk-metrics-smoke.txt || {
    printf 'metrics endpoint responded but did not include expected ldk/lightning series\n' >&2
    exit 1
  }

jq -r 'keys_unsorted[]?' /tmp/ldk-node-info.json >/dev/null 2>&1 || true
jq -r 'keys_unsorted[]?' /tmp/ldk-balances.json >/dev/null 2>&1 || true
printf 'ldk hosted readonly smoke ok: node info, balances, metrics\n'
REMOTE

log "Hosted LDK Server read-only smoke completed"
