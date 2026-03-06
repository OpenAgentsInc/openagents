#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MODULE_LIB="${1:-$ROOT_DIR/spacetime/modules/autopilot-sync/spacetimedb/src/lib.rs}"

if [[ ! -f "$MODULE_LIB" ]]; then
  echo "missing module source: $MODULE_LIB" >&2
  exit 1
fi

required_tables=(
  active_connection
  nostr_presence_claim
  stream_head
  sync_event
  stream_checkpoint
)

required_reducers=(
  init
  client_connected
  client_disconnected
  heartbeat
  request_nostr_presence_challenge
  bind_nostr_presence_identity
  append_sync_event
  ack_stream_checkpoint
)

for table in "${required_tables[@]}"; do
  if ! rg -q "table\\(name = \\\"?${table}\\\"?" "$MODULE_LIB"; then
    echo "contract verification failed: missing table '${table}'" >&2
    exit 1
  fi
done

for reducer in "${required_reducers[@]}"; do
  if ! rg -q "fn ${reducer}\(" "$MODULE_LIB"; then
    echo "contract verification failed: missing reducer '${reducer}'" >&2
    exit 1
  fi
done

echo "autopilot-sync contract verification passed"
