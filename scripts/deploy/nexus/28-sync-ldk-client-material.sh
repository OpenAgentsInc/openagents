#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd base64
require_cmd gcloud

ensure_gcloud_context
instance_exists "$NEXUS_LDK_VM" || die "LDK VM does not exist: ${NEXUS_LDK_VM}"
instance_exists "$NEXUS_VM" || die "Nexus VM does not exist: ${NEXUS_VM}"

NEXUS_LDK_CLIENT_CONFIG_DIR="${NEXUS_LDK_CLIENT_CONFIG_DIR:-/etc/nexus-relay/ldk-server}"
NEXUS_LDK_CLIENT_API_KEY_PATH="${NEXUS_LDK_CLIENT_API_KEY_PATH:-${NEXUS_LDK_CLIENT_CONFIG_DIR}/api_key}"
NEXUS_LDK_CLIENT_TLS_CERT_PATH="${NEXUS_LDK_CLIENT_TLS_CERT_PATH:-${NEXUS_LDK_CLIENT_CONFIG_DIR}/tls.crt}"
LDK_REMOTE_API_KEY_PATH="${NEXUS_LDK_STORAGE_DIR}/${NEXUS_LDK_NETWORK}/api_key"
LDK_REMOTE_TLS_CERT_PATH="${NEXUS_LDK_STORAGE_DIR}/tls.crt"

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/nexus-ldk-client-material.XXXXXX")"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT
umask 077

download_remote_secret_file() {
  local remote_path="$1"
  local local_path="$2"
  local encoded_path="${local_path}.b64"

  gcloud compute ssh "$NEXUS_LDK_VM" \
    --tunnel-through-iap \
    --project "$GCP_PROJECT" \
    --zone "$GCP_ZONE" \
    --command "sudo test -r '${remote_path}' && sudo base64 -w0 '${remote_path}'" \
    >"$encoded_path"
  [[ -s "$encoded_path" ]] || die "Remote LDK file was empty or unreadable: ${remote_path}"
  base64 -d <"$encoded_path" >"$local_path"
  [[ -s "$local_path" ]] || die "Downloaded LDK file decoded empty: ${remote_path}"
}

log "Copying LDK client material from ${NEXUS_LDK_VM} to ${NEXUS_VM}; secret bytes will not be printed."
download_remote_secret_file "$LDK_REMOTE_API_KEY_PATH" "$tmp_dir/api_key"
download_remote_secret_file "$LDK_REMOTE_TLS_CERT_PATH" "$tmp_dir/tls.crt"

gcloud compute scp "$tmp_dir/api_key" "$NEXUS_VM:/tmp/nexus-ldk-api-key" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" >/dev/null

gcloud compute scp "$tmp_dir/tls.crt" "$NEXUS_VM:/tmp/nexus-ldk-tls.crt" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" >/dev/null

# ldk-server's generated TLS certificate is valid for the VM hostname, not the
# private IP. GCE internal DNS resolves this name from the Nexus VM.
ldk_server_url="${NEXUS_LDK_VM}:${NEXUS_LDK_GRPC_PORT}"

gcloud compute ssh "$NEXUS_VM" \
  --tunnel-through-iap \
  --project "$GCP_PROJECT" \
  --zone "$GCP_ZONE" \
  --command "sudo env \
    NEXUS_LDK_CLIENT_CONFIG_DIR='${NEXUS_LDK_CLIENT_CONFIG_DIR}' \
    NEXUS_LDK_CLIENT_API_KEY_PATH='${NEXUS_LDK_CLIENT_API_KEY_PATH}' \
    NEXUS_LDK_CLIENT_TLS_CERT_PATH='${NEXUS_LDK_CLIENT_TLS_CERT_PATH}' \
    NEXUS_LDK_SERVER_URL='${ldk_server_url}' \
    NEXUS_LDK_NETWORK='${NEXUS_LDK_NETWORK}' \
    bash -s" <<'REMOTE'
set -euo pipefail

mkdir -p "$NEXUS_LDK_CLIENT_CONFIG_DIR"
chown 60000:60000 "$NEXUS_LDK_CLIENT_CONFIG_DIR"
install -m 0400 -o 60000 -g 60000 /tmp/nexus-ldk-api-key "$NEXUS_LDK_CLIENT_API_KEY_PATH"
install -m 0444 -o root -g root /tmp/nexus-ldk-tls.crt "$NEXUS_LDK_CLIENT_TLS_CERT_PATH"
rm -f /tmp/nexus-ldk-api-key /tmp/nexus-ldk-tls.crt

cat >"${NEXUS_LDK_CLIENT_CONFIG_DIR}/client.env" <<ENV
NEXUS_TREASURY_PROVIDER=ldk
NEXUS_LDK_SERVER_URL=${NEXUS_LDK_SERVER_URL}
NEXUS_LDK_API_KEY_PATH=${NEXUS_LDK_CLIENT_API_KEY_PATH}
NEXUS_LDK_TLS_CERT_PATH=${NEXUS_LDK_CLIENT_TLS_CERT_PATH}
NEXUS_LDK_NETWORK=${NEXUS_LDK_NETWORK}
NEXUS_LDK_CHAIN_BACKEND=bitcoind
ENV
chmod 0640 "${NEXUS_LDK_CLIENT_CONFIG_DIR}/client.env"
chown root:root "${NEXUS_LDK_CLIENT_CONFIG_DIR}/client.env"
REMOTE

log "LDK client material installed on ${NEXUS_VM} at ${NEXUS_LDK_CLIENT_CONFIG_DIR}"
log "Use NEXUS_LDK_SERVER_URL=${ldk_server_url} for Nexus deploy; API key and TLS cert paths are local to ${NEXUS_VM}."
