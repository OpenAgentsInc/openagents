#!/usr/bin/env bash
# The Cultist Raid against a local room server (raid-check.coax): it starts BIN (a room server
# built by build.sh) on PORT with its control lines read from a file, joins N players, presses
# START NOW by rewriting the file, and lets the raid run SECONDS. The server runs as local.sh runs
# it (--dev, room-dev.coax): its world is dev/'s, its control lines the file, its key the stand-in's,
# and nothing here reaches the API.
#   coax/web/room/raid-proof.sh BIN [N] [SECONDS] [PORT]
# RAID_CLASS (druid, warlock, mix), RAID_MODE (square, fight) and RAID_LEVEL2=1 (every player must
# reach level 2) pass to raid-check.coax.
# Exits 0 when raid-check prints "raids".
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
bin="$1"; n="${2:-8}"; secs="${3:-160}"; port="${4:-8871}"
dir="$(mktemp -d "${TMPDIR:-/tmp}/raid-proof.XXXXXX")"
ctl="$dir/room-control.txt"
cp "$here/dev/world-everglade-5e.json" "$dir/"
# the API's defaults: EVERY 0, 3 waves of 12 at 4 players (scaled to those here: everglade-raid-
# scale.coax), 6 Cultists, 1 Fanatic, 3 Warlocks in parts by the last wave, levels up to 3, all gates,
# the leader on at aggression 2, town damage on, 100% reward, 15 minutes; levels on, cap 2
dials="${RAID_DIALS:-event 1 0 3 12 6 1 3 3 31 1 2 1 100 15}"   # the API's default dials; RAID_DIALS: another raid
printf '%s 0 0\nlevels 1 2\n' "$dials" > "$ctl"
key="local-development-room-ticket-key-not-a-secret"   # room-dev.coax DEV_KEY
"$bin" --port "$port" --dev "$dir" > "$dir/room.log" 2>&1 &
room=$!
trap 'kill "$room" 2>/dev/null || true; cp "$dir/room.log" "${RAID_PROOF_LOG:-/dev/null}" 2>/dev/null || true; rm -rf "$dir"' EXIT
# a press of START acts in the live rooms only, so it waits for the players: build the check first
"$root/coax/web/tool.sh" "$root/coax/web/room/raid-check.coax" "ws://127.0.0.1:1" 1 10 > /dev/null 2>&1 || true
sleep 3
(sleep $((3 + n / 2)); printf '%s 1 0\nlevels 1 2\n' "$dials" > "$ctl") &
# the server's own health every 5 s: the Everglade tick's p50 and 95th percentile (room-host.c GET /health)
( while kill -0 "$room" 2>/dev/null; do curl -s --max-time 2 "http://127.0.0.1:$port/health" >> "$dir/health.log" || true; sleep 5; done ) &
OA_ROOM_TICKET_KEY="$key" "$root/coax/web/tool.sh" "$root/coax/web/room/raid-check.coax" "ws://127.0.0.1:$port" "$n" "$secs" "$(date +%s)" "${RAID_CLASS:-druid}" "${RAID_MODE:-square}" "${RAID_LEVEL2:-0}" | tee "$dir/check.out"
# the worst 95th percentile tick seen, and the minute reports (ticks, sends, events with no room)
worst=$(sed -n 's/.*at the 95th \([0-9]*\) us.*/\1/p' "$dir/health.log" | sort -n | tail -1)
echo "tick p95 at worst: ${worst:-?} us"
grep -h "room: .*ticks in\|tick parts" "$dir/room.log" | tail -6 || true
grep -qx raids "$dir/check.out"
