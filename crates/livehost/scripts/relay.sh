#!/usr/bin/env bash
# Run a local nostr-relay for livehost on ws://127.0.0.1:${LIVEHOST_RELAY_PORT:-7448}.
#
# Postgres and the relay key live in a state directory (default: a
# livehost-relay folder under `openagents scratch`). Ctrl-C stops both.
# Build the relay first:
#   cargo build --release -p nostr-relay
#
# Usage: crates/livehost/scripts/relay.sh [STATE_DIR]

set -euo pipefail

: "${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR to the shared target}"
state="${1:-$(openagents scratch 2>/dev/null || echo "$HOME/.openagents/scratch")/livehost-relay}"
port="${LIVEHOST_RELAY_PORT:-7448}"
pg_port="${LIVEHOST_RELAY_PG_PORT:-55448}"
relay="$CARGO_TARGET_DIR/release/nostr-relay"
[[ -x "$relay" ]] || { echo "relay.sh: build it first: cargo build --release -p nostr-relay" >&2; exit 1; }

mkdir -p "$state/socket"
[[ -d "$state/data" ]] || initdb -D "$state/data" -A trust --no-locale -E UTF8 >/dev/null
pg_ctl -D "$state/data" status >/dev/null 2>&1 || pg_ctl -D "$state/data" -l "$state/postgres.log" \
  -o "-c listen_addresses='127.0.0.1' -c port=$pg_port -c unix_socket_directories='$state/socket'" \
  -w start >/dev/null
createdb -h "$state/socket" -p "$pg_port" -U "$(id -un)" livehost_relay 2>/dev/null || true
[[ -f "$state/relay.key" ]] || (umask 077; od -An -tx1 -N32 /dev/urandom | tr -d ' \n' > "$state/relay.key")

relay_pid=""
stop() {
  [[ -n "$relay_pid" ]] && kill "$relay_pid" 2>/dev/null || true
  pg_ctl -D "$state/data" -m fast stop >/dev/null 2>&1 || true
}
trap stop EXIT

echo "relay.sh: ws://127.0.0.1:$port (state in $state)"
DATABASE_URL="host=127.0.0.1 port=$pg_port user=$(id -un) dbname=livehost_relay" \
NOSTR_RELAY_BIND_ADDR=127.0.0.1 \
NOSTR_RELAY_PORT="$port" \
NOSTR_RELAY_URL="ws://127.0.0.1:$port" \
NOSTR_RELAY_SECRET_KEY="$(cat "$state/relay.key")" \
NOSTR_RELAY_AUTH_REQUIRED=false \
NOSTR_RELAY_RATE_EVENTS_PER_MIN_IP=60000 \
NOSTR_RELAY_RATE_EVENTS_PER_MIN_PUBKEY=30000 \
NOSTR_RELAY_RATE_REQ_PER_MIN_IP=60000 \
NOSTR_RELAY_LOG_LEVEL="${NOSTR_RELAY_LOG_LEVEL:-warn}" \
  "$relay" &
relay_pid=$!
wait "$relay_pid"
