#!/usr/bin/env bash
# The Cultist Raid against a local room server (raid-check.coax): it starts BIN (a room server
# built by build.sh) on PORT with its control lines read from a file, joins N players, presses
# START NOW by rewriting the file, and lets the raid run SECONDS. The key is a local one made up
# here: the server needs a key to ask for its control lines, and nothing here reaches the API.
#   coax/web/room/raid-proof.sh BIN [N] [SECONDS] [PORT]
# Exits 0 when raid-check prints "raids".
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
bin="$1"; n="${2:-8}"; secs="${3:-160}"; port="${4:-8871}"
dir="$(mktemp -d "${TMPDIR:-/tmp}/raid-proof.XXXXXX")"
ctl="$dir/control.txt"
# EVERY 0, 2 waves of 12 (6 Cultists, 1 Fanatic, 3 Warlocks in parts, levels 1 to 3), all gates,
# the leader on at aggression 2, town damage on, 100% reward, 15 minutes; levels on, cap 2
dials="${RAID_DIALS:-event 1 0 2 12 6 1 3 3 31 1 2 1 100 15}"   # RAID_DIALS: another raid
printf '%s 0 0\nlevels 1 2\n' "$dials" > "$ctl"
key="raid-proof-local-key-$$-$RANDOM-0000000000000000"   # 32 or more, as the server needs
OA_ROOM_TICKET_KEY="$key" OA_ROOM_TICKET_REQUIRED=0 OA_ROOM_CONTROL_URL="file://$ctl" OA_ROOM_API="http://127.0.0.1:9" \
  "$bin" --port "$port" > "$dir/room.log" 2>&1 &
room=$!
trap 'kill "$room" 2>/dev/null || true; cp "$dir/room.log" "${RAID_PROOF_LOG:-/dev/null}" 2>/dev/null || true; rm -rf "$dir"' EXIT
# a press of START acts in the live rooms only, so it waits for the players: build the check first
"$root/coax/web/tool.sh" "$root/coax/web/room/raid-check.coax" "ws://127.0.0.1:1" 1 10 > /dev/null 2>&1 || true
sleep 3
(sleep $((3 + n / 2)); printf '%s 1 0\nlevels 1 2\n' "$dials" > "$ctl") &
OA_ROOM_TICKET_KEY="$key" "$root/coax/web/tool.sh" "$root/coax/web/room/raid-check.coax" "ws://127.0.0.1:$port" "$n" "$secs" | tee "$dir/check.out"
grep -qx raids "$dir/check.out"
