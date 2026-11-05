#!/usr/bin/env bash
# The WebSocket profile under bad networks (GitHub #12858): a local room server (BIN, built by
# build.sh) on PORT, run as local.sh runs it (--dev), the bad network in front of it on PORT + 1
# (net-proxy.coax) and load.coax's everglade players through it, once per profile. Each run prints
# load.coax's JSON line: the input-to-ACK tail (ack_p99_ms) and the state's arrival gaps
# (d_gap_p99_ms, d_gap_max_ms, d_stalls_100ms), the stalls a lost TCP packet causes.
#   coax/web/room/transport-proof.sh BIN [PORT] [PLAYERS] [SECONDS] [PROFILE...]
# A PROFILE is LATENCY_MS/JITTER_MS/LOSS_PERMILLE; the defaults are the drift check's: 0/0/0,
# 100/30/20 and 250/30/20. PLAYERS at most 6 (the proxy's sockets), default 4; SECONDS default 60.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
bin="$1"; port="${2:-8981}"; players="${3:-4}"; seconds="${4:-60}"
shift $(( $# < 4 ? $# : 4 ))
profiles=("$@"); [ ${#profiles[@]} -gt 0 ] || profiles=(0/0/0 100/30/20 250/30/20)
proxy=$((port + 1))
cd "$root"   # load.coax starts its rooms from the repository's root
for p in 8830 8843 8872 8876 8913; do
  if [ "$port" = "$p" ] || [ "$proxy" = "$p" ]; then echo "transport-proof: port $p is in use by the site's tools" >&2; exit 2; fi
done
dir="$(mktemp -d "${TMPDIR:-/tmp}/transport-proof.XXXXXX")"
cp "$here/dev/world-everglade.json" "$here/dev/room-control.txt" "$dir/"
key="local-development-room-ticket-key-not-a-secret"   # room-dev.coax DEV_KEY
"$bin" --port "$port" --dev "$dir" > "$dir/room.log" 2>&1 &
room=$!
proxy_pid=""
trap 'kill "$room" 2>/dev/null || true; [ -n "$proxy_pid" ] && kill "$proxy_pid" 2>/dev/null || true; rm -rf "$dir"' EXIT
# build the tools first, so the clock starts with the players
"$root/coax/web/tool.sh" "$here/net-proxy.coax" 1 1 0 0 0 1 > /dev/null 2>&1 || true
sleep 1
for prof in "${profiles[@]}"; do
  IFS=/ read -r lat jit loss <<< "$prof"
  "$root/coax/web/tool.sh" "$here/net-proxy.coax" "$proxy" "$port" "$lat" "$jit" "$loss" $((seconds + 30)) > "$dir/proxy.log" 2>&1 &
  proxy_pid=$!
  sleep 1
  echo "transport-proof: ${lat} ms round trip, ${jit} ms jitter, ${loss} per thousand lost, $players players, ${seconds} s"
  OA_ROOM_TICKET_KEY="$key" "$root/coax/web/tool.sh" "$here/load.coax" everglade "ws://127.0.0.1:$proxy" "$players" "$seconds" | tee "$dir/load.out" || true
  kill "$proxy_pid" 2>/dev/null || true; wait "$proxy_pid" 2>/dev/null || true; proxy_pid=""
  grep -h "lost\|reads" "$dir/proxy.log" | tail -1 || true
done
