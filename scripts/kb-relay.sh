#!/usr/bin/env bash
# Start a local nostr-relay for the shared knowledge base on ws://127.0.0.1:7490.
#
# `microcoder kb publish` and `kb sync` read and write NIP-KB events
# (kinds 3190, 30190, and 3191) through it. Postgres state lives in a
# directory that persists between runs, so published entries stay.
#
# Usage:
#   scripts/kb-relay.sh               # start in the foreground; Ctrl-C stops it
#   KB_RELAY_DIR=/path scripts/kb-relay.sh   # choose the state directory
#   scripts/kb-relay.sh --reset       # wipe the stored events first
#
# Requires initdb, pg_ctl, createdb, and cargo on PATH.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state="${KB_RELAY_DIR:-${HOME}/.openagents/knowledge/relay}"
port="${KB_RELAY_PORT:-7490}"
pg_port="${KB_RELAY_PG_PORT:-55490}"

for tool in initdb pg_ctl createdb cargo; do
  command -v "$tool" >/dev/null || { echo "kb-relay: $tool is not on PATH" >&2; exit 1; }
done

if [[ "${1:-}" == "--reset" ]]; then
  pg_ctl -D "$state/data" -m fast stop >/dev/null 2>&1 || true
  rm -rf "$state"
fi

mkdir -p "$state/socket"
if [[ ! -d "$state/data" ]]; then
  initdb -D "$state/data" -A trust --no-locale -E UTF8 >/dev/null
fi
if ! pg_ctl -D "$state/data" status >/dev/null 2>&1; then
  pg_ctl -D "$state/data" -l "$state/postgres.log" \
    -o "-c listen_addresses='127.0.0.1' -c port=$pg_port -c unix_socket_directories='$state/socket'" \
    -w start >/dev/null
fi
createdb -h "$state/socket" -p "$pg_port" -U "$(id -un)" kb_relay 2>/dev/null || true

# A development signer for the relay's own metadata, created once.
if [[ ! -f "$state/relay.key" ]]; then
  (umask 077; od -An -tx1 -N32 /dev/urandom | tr -d ' \n' > "$state/relay.key")
fi

cargo build -q --release -p nostr-relay --bin nostr-relay --manifest-path "$root/Cargo.toml"

relay_pid=""
stop() {
  [[ -n "$relay_pid" ]] && kill "$relay_pid" 2>/dev/null || true
  pg_ctl -D "$state/data" -m fast stop >/dev/null 2>&1 || true
}
trap stop EXIT

echo "kb-relay: ws://127.0.0.1:$port (state in $state)"
DATABASE_URL="host=127.0.0.1 port=$pg_port user=$(id -un) dbname=kb_relay" \
NOSTR_RELAY_BIND_ADDR=127.0.0.1 \
NOSTR_RELAY_PORT="$port" \
NOSTR_RELAY_URL="ws://127.0.0.1:$port" \
NOSTR_RELAY_SECRET_KEY="$(cat "$state/relay.key")" \
NOSTR_RELAY_AUTH_REQUIRED=false \
NOSTR_RELAY_LOG_LEVEL="${NOSTR_RELAY_LOG_LEVEL:-warn}" \
  "$root/target/release/nostr-relay" &
relay_pid=$!
wait "$relay_pid"
