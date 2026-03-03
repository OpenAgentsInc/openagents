#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/common.sh"

require_cmd gcloud

ensure_gcloud_context

NEW_RPC_USER="${NEW_RPC_USER:-}"
NEW_RPC_PASSWORD="${NEW_RPC_PASSWORD:-}"
BITCOIND_UPDATED="${BITCOIND_UPDATED:-0}"

[[ -n "$NEW_RPC_USER" ]] || die "Set NEW_RPC_USER"
[[ -n "$NEW_RPC_PASSWORD" ]] || die "Set NEW_RPC_PASSWORD"
[[ "$BITCOIND_UPDATED" == "1" ]] || die "Set BITCOIND_UPDATED=1 only after bitcoind is updated with matching credentials"

TMP_FILE="$(mktemp)"
trap 'rm -f "$TMP_FILE"' EXIT
printf 'rpcuser=%s\nrpcpassword=%s\n' "$NEW_RPC_USER" "$NEW_RPC_PASSWORD" >"$TMP_FILE"

gcloud secrets versions add "$BITCOIND_RPC_SECRET" \
  --project "$GCP_PROJECT" \
  --data-file "$TMP_FILE" >/dev/null

log "Added new secret version for ${BITCOIND_RPC_SECRET}. Reconfiguring Symphony service."
"${SCRIPT_DIR}/03-configure-and-start.sh"

log "RPC credential rotation workflow complete"
