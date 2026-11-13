#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: starts the room binary, runs the Coax bots against it, reads their totals).
# The playable loop in a local --dev room (#13381, #13293): bots sit as players do, take the Warden's
# quest, kill wolves from the content tables' spawns, are told their drops (granted through the item
# ledger's stand-in), hand the quest in and are paid once. Fails unless the bots' totals show each step.
#   coax/remote/remote.sh run -- everglade/tools/loop-proof.sh [PLAYERS] [SECONDS] [full]
# In 240 s a bot takes the quest, kills and is told its drops. With `full` (give it 780 s or more) the
# run must also show a quest handed in: its reward paid, its coin, and the cloak put on.
# Prints the bots' totals line and "loop-proof: holds" or what was missing. Builds the
# room from this checkout each time.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"; root="$(cd "$here/../.." && pwd -P)"
cd "$root"
players="${1:-4}"; secs="${2:-240}"; full="${3:-}"
key="local-development-room-ticket-key-not-a-secret"
work="$(mktemp -d "${TMPDIR:-/tmp}/loop-proof.XXXXXX")"; pid=""
# this checkout's room, built now: a binary left by another commit plays other tables and another wire
coax/web/room/build.sh "$work/room" > "$work/build.log" 2>&1 || { tail -20 "$work/build.log"; echo "loop-proof: the room did not build"; exit 1; }
trap '[[ -z "$pid" ]] || kill "$pid" 2> /dev/null; rm -rf "$work"' EXIT
port=0
for n in 1 2 3 4 5 6 7 8; do port=$(( 20000 + RANDOM % 20000 )); curl -s -m 1 -o /dev/null "http://127.0.0.1:$port/" 2> /dev/null || break; done
OA_ROOM_TICKET_KEY="$key" "$work/room" --port "$port" --dev "$root/coax/web/room/dev" > "$work/room.log" 2>&1 & pid=$!
for n in $(seq 1 100); do curl -fs "http://127.0.0.1:$port/health" > /dev/null 2>&1 && break; read -r -t 0.1 < /dev/zero || true; done
OA_ROOM_TICKET_KEY="$key" coax/web/tool.sh coax/web/room/bots.coax run "ws://localhost:$port" "$players" "$secs" --direct --zone --realm pair-e2-loop > "$work/bots.log" 2>&1
grep -m 3 -i "content" "$work/room.log" | sed 's/^/room: /'
totals="$(grep '^bots ' "$work/bots.log" | tail -1)"
[[ -n "$totals" ]] || { tail -20 "$work/bots.log"; echo "loop-proof: the bots printed no totals"; exit 1; }
echo "$totals"
num() { echo "$totals" | grep -o "\"$1\":[0-9]*" | head -1 | cut -d: -f2; }
missing=""
want="seated says quest_stages kills loot items"
[[ "$full" != full ]] || want="$want quests_paid coin worn"
for k in $want; do
  v="$(num "$k")"; [[ -n "$v" && "$v" -gt 0 ]] || missing="$missing $k"
done
# a reward is paid once a quest: never more quests paid than bots times the tables' two quests
paid="$(num quests_paid)"; [[ -z "$paid" || "$paid" -le $(( players * 2 )) ]] || missing="$missing paid-twice"
[[ -z "$missing" ]] || { echo "loop-proof: missing:$missing"; exit 1; }
echo "loop-proof: holds ($players bots, $secs s${full:+, the hand-in too})"
