#!/usr/bin/env bash
# The cast rules under latency (cast-check.coax, GitHub #12790): a local room server (BIN, built by
# build.sh) on PORT, run as local.sh runs it (--dev: dev/'s world, a control file, the stand-in's
# ticket key), the bad network in front of it on PORT + 1 (net-proxy.coax: LATENCY_MS round trip,
# JITTER_MS, LOSS_PERMILLE of reads lost), and cast-check's four players through it.
#   coax/web/room/cast-proof.sh BIN [PORT] [LATENCY_MS] [JITTER_MS] [LOSS_PERMILLE] [ROUNDS]
# Defaults: 8881, 250 ms, 30 ms, 20 (2%), 6 rounds. Exits 0 when cast-check prints "casts hold".
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
bin="$1"; port="${2:-8881}"; lat="${3:-250}"; jit="${4:-30}"; loss="${5:-20}"; rounds="${6:-6}"
proxy=$((port + 1))
dir="$(mktemp -d "${TMPDIR:-/tmp}/cast-proof.XXXXXX")"
cp "$here/dev/world-everglade.json" "$dir/"
printf 'levels 0 2\n' > "$dir/room-control.txt"   # levels off: every seat has its whole bar
key="local-development-room-ticket-key-not-a-secret"   # room-dev.coax DEV_KEY
"$bin" --port "$port" --dev "$dir" > "$dir/room.log" 2>&1 &
room=$!
proxy_pid=""
trap 'kill "$room" 2>/dev/null || true; [ -n "$proxy_pid" ] && kill "$proxy_pid" 2>/dev/null || true; cp "$dir/room.log" "${CAST_PROOF_LOG:-/dev/null}" 2>/dev/null || true; rm -rf "$dir"' EXIT
# build the tools first, so the clock starts with the players
"$root/coax/web/tool.sh" "$here/net-proxy.coax" 1 1 0 0 0 1 > /dev/null 2>&1 || true
"$root/coax/web/tool.sh" "$here/cast-check.coax" > /dev/null 2>&1 || true
"$root/coax/web/tool.sh" "$here/net-proxy.coax" "$proxy" "$port" "$lat" "$jit" "$loss" 600 > "$dir/proxy.log" 2>&1 &
proxy_pid=$!
sleep 2
echo "cast-proof: room on $port, the network on $proxy: ${lat} ms round trip, ${jit} ms jitter, ${loss} per thousand lost"
OA_ROOM_TICKET_KEY="$key" "$root/coax/web/tool.sh" "$here/cast-check.coax" "ws://127.0.0.1:$proxy" "$(date +%s)" "$rounds" | tee "$dir/check.out"
grep -h "lost\|reads" "$dir/proxy.log" | tail -2 || true
grep -qx "casts hold" "$dir/check.out"
