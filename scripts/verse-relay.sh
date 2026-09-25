#!/usr/bin/env bash
# Start a local nostr-relay for Verse multiplayer on ws://127.0.0.1:7447.
#
# Verse streams NIP-MV pose frames about 10 times a second per player. The
# relay's default limits (60 events a minute per pubkey, 120 per IP) are
# meant for ordinary Nostr traffic, so this script raises them for local
# play. Postgres state lives in a scratch directory that persists between
# runs, so avatars resume where they were left.
#
# Usage:
#   scripts/verse-relay.sh            # start in the foreground; Ctrl-C stops it
#   VERSE_RELAY_BIND=0.0.0.0 scripts/verse-relay.sh   # let other machines join
#   VERSE_RELAY_DIR=/path scripts/verse-relay.sh      # choose the state directory
#   scripts/verse-relay.sh --reset    # wipe the stored world first
#
# It also creates Verse's NIP-29 chat rooms as the relay.
#
# Requires initdb, pg_ctl, createdb, and curl on PATH.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
state="${VERSE_RELAY_DIR:-${HOME}/.openagents/verse/relay}"
port="${VERSE_RELAY_PORT:-7447}"
pg_port="${VERSE_RELAY_PG_PORT:-55447}"
bind="${VERSE_RELAY_BIND:-127.0.0.1}"

for tool in initdb pg_ctl createdb cargo; do
  command -v "$tool" >/dev/null || { echo "verse-relay: $tool is not on PATH" >&2; exit 1; }
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
createdb -h "$state/socket" -p "$pg_port" -U "$(id -un)" verse_relay 2>/dev/null || true

# A development signer for the relay's own metadata, created once.
if [[ ! -f "$state/relay.key" ]]; then
  (umask 077; od -An -tx1 -N32 /dev/urandom | tr -d ' \n' > "$state/relay.key")
fi

cargo build -q --release -p nostr-relay --bin nostr-relay -p verse --bin verse \
  --manifest-path "$root/Cargo.toml"

relay_pid=""
stop() {
  [[ -n "$relay_pid" ]] && kill "$relay_pid" 2>/dev/null || true
  pg_ctl -D "$state/data" -m fast stop >/dev/null 2>&1 || true
}
trap stop EXIT

echo "verse-relay: ws://$bind:$port (state in $state)"
DATABASE_URL="host=127.0.0.1 port=$pg_port user=$(id -un) dbname=verse_relay" \
NOSTR_RELAY_BIND_ADDR="$bind" \
NOSTR_RELAY_PORT="$port" \
NOSTR_RELAY_URL="ws://$bind:$port" \
NOSTR_RELAY_SECRET_KEY="$(cat "$state/relay.key")" \
NOSTR_RELAY_AUTH_REQUIRED=false \
NOSTR_RELAY_RATE_EVENTS_PER_MIN_IP=60000 \
NOSTR_RELAY_RATE_EVENTS_PER_MIN_PUBKEY=30000 \
NOSTR_RELAY_RATE_REQ_PER_MIN_IP=6000 \
NOSTR_RELAY_LOG_LEVEL="${NOSTR_RELAY_LOG_LEVEL:-warn}" \
  "$root/target/release/nostr-relay" &
relay_pid=$!

# Create the NIP-29 chat rooms (lounge, trading-post, builders) once the
# relay answers. Only the relay's own key may create groups.
for _ in $(seq 1 100); do
  curl -sf "http://127.0.0.1:$port/" -H 'Accept: application/nostr+json' >/dev/null && break
  sleep 0.1
done
"$root/target/release/verse" --seed-rooms "$state/relay.key" --relay "ws://127.0.0.1:$port" || true

wait "$relay_pid"
