#!/usr/bin/env bash
# The Grid soak (#10589): simulated walkers, a viewer that times every frame
# it receives, and simulated phone, desktop, and browser players drawing the
# Grid, all on a scratch relay with the production world limits.
#
# Usage:
#   scripts/grid-soak.sh [--seconds 1800] [--walkers 17] [--out DIR]
#                        [--world verse-everglade]
#
# Everything runs under a temporary directory with a temporary HOME: a
# scratch Postgres, the relay on ws://127.0.0.1:7457, and fresh in-memory
# keys. Nothing reaches a public relay, the keychain, or the real home.
#
# The relay keeps its defaults for each key and each world (the pose lane,
# the 20-player population cap, and the world frame budget). It raises only
# the per-address limits, because every simulated player shares one
# address here, where real players each have their own.
#
# 17 walkers and the three platform clients make 20 players, the default
# cap. Halfway through, one more walker joins to check the cap refuses it.
#
# Receipts land in DIR (default bench/verse/<date>/grid-soak): meta.json,
# walkers.ndjson, load.json, frames.ndjson, summary.json, cap.ndjson,
# relay.log, and each client's first and last view as a PNG. The PNGs and
# any receipt over 1 MB go to the bench bucket afterwards
# (scripts/bench-artifacts.py push DIR), which leaves a sha256 manifest in
# DIR and keeps them out of git; without gcloud, run that push by hand
# before committing the receipts.
#
# `--world` soaks a portal's shared zone instance (`verse-everglade`,
# `verse-lagrange-1`) instead: walkers stand in for all 20 players there,
# since the rendering clients draw the Grid only.
#
# To run with real devices instead, start the relay the same way, point
# the phone, `verse --frame-times`, and the browser at it, and leave out the
# simulated clients; NEEDS_OWNER.md lists that run.
#
# Requires initdb, pg_ctl, createdb, and cargo on PATH.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
seconds=1800
walkers=17
out=""
world=verse-bare
while [[ $# -gt 0 ]]; do
  case "$1" in
    --seconds) seconds="$2"; shift 2 ;;
    --walkers) walkers="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    --world) world="$2"; shift 2 ;;
    *) echo "grid-soak: unknown argument $1" >&2; exit 2 ;;
  esac
done
out="${out:-$root/bench/verse/$(date +%F)/grid-soak}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
players=$((walkers + 3))
if [[ "$world" != verse-bare ]]; then
  walkers=$players
fi
port="${GRID_SOAK_PORT:-7457}"
pg_port="${GRID_SOAK_PG_PORT:-55457}"
target="${CARGO_TARGET_DIR:-$root/target}"

for tool in initdb pg_ctl createdb cargo; do
  command -v "$tool" >/dev/null || { echo "grid-soak: $tool is not on PATH" >&2; exit 1; }
done

cargo build -q --release --manifest-path "$root/Cargo.toml" \
  -p nostr-relay --bin nostr-relay -p openagents-cli --bin openagents
cargo build -q --release --manifest-path "$root/Cargo.toml" -p verse --example grid_soak

scratch="$(mktemp -d "${TMPDIR:-/tmp}/grid-soak.XXXXXX")"
mkdir -p "$scratch/home" "$scratch/socket"
pids=()
stop() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
  pg_ctl -D "$scratch/data" -m fast stop >/dev/null 2>&1 || true
  rm -rf "$scratch"
}
trap stop EXIT

initdb -D "$scratch/data" -A trust --no-locale -E UTF8 >/dev/null
pg_ctl -D "$scratch/data" -l "$scratch/postgres.log" \
  -o "-c listen_addresses='127.0.0.1' -c port=$pg_port -c unix_socket_directories='$scratch/socket'" \
  -w start >/dev/null
createdb -h "$scratch/socket" -p "$pg_port" -U "$(id -un)" grid_soak
relay_key="$(od -An -tx1 -N32 /dev/urandom | tr -d ' \n')"
relay="ws://127.0.0.1:$port"

HOME="$scratch/home" \
DATABASE_URL="host=127.0.0.1 port=$pg_port user=$(id -un) dbname=grid_soak" \
NOSTR_RELAY_BIND_ADDR=127.0.0.1 \
NOSTR_RELAY_PORT="$port" \
NOSTR_RELAY_URL="$relay" \
NOSTR_RELAY_SECRET_KEY="$relay_key" \
NOSTR_RELAY_AUTH_REQUIRED=false \
NOSTR_RELAY_MAX_CONNECTIONS_PER_IP=200 \
NOSTR_RELAY_RATE_EVENTS_PER_MIN_IP=60000 \
NOSTR_RELAY_RATE_REQ_PER_MIN_IP=6000 \
NOSTR_RELAY_RATE_POSE_PER_SEC_IP=4000 \
NOSTR_RELAY_LOG_LEVEL=warn \
  "$target/release/nostr-relay" >"$out/relay.log" 2>&1 &
pids+=($!)
for _ in $(seq 1 100); do
  curl -sf "http://127.0.0.1:$port/" -H 'Accept: application/nostr+json' >/dev/null && break
  sleep 0.1
done

cat >"$out/meta.json" <<EOF
{
  "issue": 10589,
  "revision": "$(git -C "$root" rev-parse HEAD)",
  "dirty": $(if [[ -n "$(git -C "$root" status --porcelain -- crates scripts)" ]]; then echo true; else echo false; fi),
  "relay": "$relay (scratch nostr-relay, default world limits, per-address limits raised)",
  "world": "$world",
  "players": $players,
  "walkers": $walkers,
  "clients": $(if [[ "$world" == verse-bare ]]; then echo '["phone (simulated)", "desktop (simulated)", "browser (simulated)"]'; else echo '[]'; fi),
  "seconds": $seconds,
  "leases": "${OPENAGENTS_LEASES:-}",
  "host": "$(uname -sm), $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -p)",
  "started": "$(date -u +%FT%TZ)"
}
EOF

oa="$target/release/openagents"
HOME="$scratch/home" "$oa" verse walkers "$walkers" --relay "$relay" --world "$world" \
  --wait $((seconds + 120)) --json >"$out/walkers.ndjson" 2>&1 &
pids+=($!)
sleep 10
HOME="$scratch/home" "$oa" verse load --relay "$relay" --world "$world" --players "$players" \
  --max-age-ms 1000 --wait "$seconds" --json >"$out/load.json" 2>&1 &
load_pid=$!
pids+=($load_pid)

# Halfway through, a 21st player tries to join the full world; the relay
# must refuse it while the 20 play on.
(sleep $((seconds / 2)); HOME="$scratch/home" "$oa" verse walkers 1 --relay "$relay" --world "$world" \
  --wait 20 --json >"$out/cap.ndjson" 2>&1) &
pids+=($!)

set +e
clients=0
if [[ "$world" == verse-bare ]]; then
  HOME="$scratch/home" "$target/release/examples/grid_soak" --relay "$relay" \
    --seconds "$seconds" --out "$out" 2>"$out/clients.log"
  clients=$?
fi
wait "$load_pid"
load=$?
set -e

echo "grid-soak: clients exit $clients, load exit $load; receipts in $out"
if ! { command -v gcloud >/dev/null && python3 "$root/scripts/bench-artifacts.py" push "$out"; }; then
  echo "grid-soak: captures not uploaded; run scripts/bench-artifacts.py push $out before committing" >&2
fi
[[ $clients -eq 0 && $load -eq 0 ]]
