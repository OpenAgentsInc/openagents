#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMMON_SCRIPT="${SCRIPT_DIR}/common.sh"
TOPOLOGY_SCRIPT="${SCRIPT_DIR}/22-provision-ldk-topology.sh"
INSTALL_SCRIPT="${SCRIPT_DIR}/23-install-ldk-server-host.sh"
SMOKE_SCRIPT="${SCRIPT_DIR}/24-smoke-ldk-server-readonly.sh"
BACKUP_SCRIPT="${SCRIPT_DIR}/25-backup-ldk-server-state.sh"
RESTORE_SCRIPT="${SCRIPT_DIR}/26-restore-ldk-server-drill.sh"
READINESS_SCRIPT="${SCRIPT_DIR}/27-smoke-ldk-production-readiness.sh"

assert_contains() {
  local needle="$1"
  local haystack="$2"
  if ! grep -Fq -- "$needle" <<<"$haystack"; then
    printf 'missing expected content: %s\n' "$needle" >&2
    exit 1
  fi
}

assert_not_contains() {
  local needle="$1"
  local haystack="$2"
  if grep -Fq -- "$needle" <<<"$haystack"; then
    printf 'unexpected content found: %s\n' "$needle" >&2
    exit 1
  fi
}

bash -n "$TOPOLOGY_SCRIPT" "$INSTALL_SCRIPT" "$SMOKE_SCRIPT" "$BACKUP_SCRIPT" "$RESTORE_SCRIPT" "$READINESS_SCRIPT"

COMMON_TEXT="$(cat "$COMMON_SCRIPT")"
TOPOLOGY_TEXT="$(cat "$TOPOLOGY_SCRIPT")"
INSTALL_TEXT="$(cat "$INSTALL_SCRIPT")"
SMOKE_TEXT="$(cat "$SMOKE_SCRIPT")"
BACKUP_TEXT="$(cat "$BACKUP_SCRIPT")"
RESTORE_TEXT="$(cat "$RESTORE_SCRIPT")"
READINESS_TEXT="$(cat "$READINESS_SCRIPT")"

assert_contains 'NEXUS_LDK_VM' "$COMMON_TEXT"
assert_contains 'NEXUS_LDK_GRPC_PORT' "$COMMON_TEXT"
assert_contains 'NEXUS_LDK_REMOTE_SMOKE' "$COMMON_TEXT"
assert_contains 'NEXUS_BITCOIND_RPC_FIREWALL_RULE' "$COMMON_TEXT"
assert_contains 'NEXUS_LDK_PRIVATE_P2P_FIREWALL_RULE' "$COMMON_TEXT"
assert_contains 'NEXUS_LDK_PRIVATE_P2P_SOURCE_TAGS' "$COMMON_TEXT"

assert_contains 'no-address' "$TOPOLOGY_TEXT"
assert_contains '--source-tags "$NEXUS_TAG"' "$TOPOLOGY_TEXT"
assert_contains '--source-tags "$NEXUS_LDK_TAG"' "$TOPOLOGY_TEXT"
assert_contains '--source-tags "$NEXUS_LDK_PRIVATE_P2P_SOURCE_TAGS"' "$TOPOLOGY_TEXT"
assert_contains 'NEXUS_LDK_ALLOW_PUBLIC_P2P' "$TOPOLOGY_TEXT"

assert_contains 'ldk-server.service' "$INSTALL_TEXT"
assert_contains 'Restart=always' "$INSTALL_TEXT"
assert_contains 'logrotate' "$INSTALL_TEXT"
assert_contains 'grpc_service_address = "0.0.0.0:${NEXUS_LDK_GRPC_PORT}"' "$INSTALL_TEXT"
assert_contains 'api_key' "$INSTALL_TEXT"
assert_contains 'tls.crt' "$INSTALL_TEXT"
assert_not_contains 'xxd -p' "$INSTALL_TEXT"

assert_contains 'get-node-info' "$SMOKE_TEXT"
assert_contains 'get-balances' "$SMOKE_TEXT"
assert_contains '/metrics' "$SMOKE_TEXT"
assert_contains 'cargo test -p nexus-control ldk_server' "$SMOKE_TEXT"

assert_contains 'keys_seed' "$BACKUP_TEXT"
assert_contains 'ldk_node_data.sqlite' "$BACKUP_TEXT"
assert_contains 'gcloud compute snapshots create' "$BACKUP_TEXT"
assert_contains 'contains_secret_material' "$BACKUP_TEXT"

assert_contains 'mount -o ro' "$RESTORE_TEXT"
assert_contains 'NEXUS_LDK_RESTORE_SNAPSHOT' "$RESTORE_TEXT"
assert_contains 'ldk_node_data.sqlite' "$RESTORE_TEXT"

assert_contains '/v1/treasury/status' "$READINESS_TEXT"
assert_contains '/v1/treasury/funding-target' "$READINESS_TEXT"
assert_contains '/v1/admin/treasury/operations' "$READINESS_TEXT"
assert_contains 'treasury.listChannels' "$READINESS_TEXT"
assert_contains 'treasury.openChannel' "$READINESS_TEXT"
assert_contains 'NEXUS_LDK_SMOKE_MIN_CHANNEL_SATS' "$READINESS_TEXT"
assert_not_contains 'provider_target' "$READINESS_TEXT"

TOPOLOGY_OUTPUT="$(
  NEXUS_LDK_TOPOLOGY_DRY_RUN=true \
  bash "$TOPOLOGY_SCRIPT" 2>&1
)"
assert_contains 'gcloud compute instances create' "$TOPOLOGY_OUTPUT"
assert_contains 'no-address' "$TOPOLOGY_OUTPUT"
assert_contains 'oa-allow-nexus-ldk-host-grpc-private' "$TOPOLOGY_OUTPUT"
assert_contains 'oa-allow-nexus-ldk-host-p2p-private' "$TOPOLOGY_OUTPUT"
assert_contains '--source-tags nexus-host' "$TOPOLOGY_OUTPUT"
assert_contains '--source-tags oa-lnd' "$TOPOLOGY_OUTPUT"
assert_contains 'Skipping public Lightning P2P firewall' "$TOPOLOGY_OUTPUT"
assert_not_contains '--source-ranges 0.0.0.0/0' "$TOPOLOGY_OUTPUT"

INSTALL_OUTPUT="$(
  NEXUS_LDK_INSTALL_DRY_RUN=true \
  bash "$INSTALL_SCRIPT" 2>&1
)"
assert_contains 'Would install on nexus-ldk-mainnet-1' "$INSTALL_OUTPUT"
assert_contains 'remote install must run as root' "$INSTALL_OUTPUT"

BACKUP_OUTPUT="$(
  NEXUS_LDK_BACKUP_DRY_RUN=true \
  bash "$BACKUP_SCRIPT" 2>&1
)"
assert_contains 'gcloud compute snapshots create' "$BACKUP_OUTPUT"
assert_contains 'ldk-server-state.tar.gz' "$BACKUP_OUTPUT"

RESTORE_OUTPUT="$(
  NEXUS_LDK_RESTORE_DRY_RUN=true \
  NEXUS_LDK_RESTORE_SNAPSHOT=dry-run-snapshot \
  bash "$RESTORE_SCRIPT" 2>&1
)"
assert_contains 'gcloud compute disks create' "$RESTORE_OUTPUT"

printf 'ok: nexus ldk topology deploy lane has private-network, install, smoke, backup, and restore guards\n'
