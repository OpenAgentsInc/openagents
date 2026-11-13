#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: starts the room binary, runs the Coax bots against it, reads their totals).
# The playable loop in local --dev rooms (#13381, #13293): bots sit as players do and go where their
# quests send them (bots.coax --quest reads the content tables as a quest log does). They take the
# Warden's quest, kill wolves from the tables' spawns, are told their drops (granted through the item
# ledger's stand-in), hand the quest in and are paid once, and take the next link of the chain.
# Fails unless the bots' totals show each step.
#   coax/remote/remote.sh run -- everglade/tools/loop-proof.sh [PLAYERS] [SECONDS] [full]
# Three runs, each in its own room, at once:
#   party   PLAYERS bots together: seated, a line heard, quest stages, kills, drops, items. With `full`
#           (give it 780 s or more) also a quest handed in: its reward paid, its coin, the cloak put on.
#   fallen  one level 1 Wizard who walks into the wolf packs alone and presses nothing (--patrol 330
#           --meek), and so falls. The room
#           rolls its Death Saving Throws and wakes it whole where it first stood (progress.coax
#           ev_rise): the run must show a down and a wake.
#   solo    (`full` only) one bot alone for each class in LOOP_CLASSES (default 4 11 8: Fighter, Wizard,
#           Rogue). Each must be paid the first quest and the road south, and so reach level 3; its
#           line says how far down the chain it got (quests paid, level, downs, wakes).
# Prints each run's totals and "loop-proof: holds" or what was missing. Builds the room from this
# checkout each time.
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"; root="$(cd "$here/../.." && pwd -P)"
cd "$root"
players="${1:-4}"; secs="${2:-240}"; full="${3:-}"
key="local-development-room-ticket-key-not-a-secret"
work="$(mktemp -d "${TMPDIR:-/tmp}/loop-proof.XXXXXX")"
# this checkout's room, built now: a binary left by another commit plays other tables and another wire
coax/web/room/build.sh "$work/room" > "$work/build.log" 2>&1 || { tail -20 "$work/build.log"; echo "loop-proof: the room did not build"; exit 1; }
classes="${LOOP_CLASSES:-4 11 8}"
names=(Barbarian Bard Cleric Druid Fighter Monk Paladin Ranger Rogue Sorcerer Warlock Wizard)
pids=""
trap 'for p in $pids; do kill "$p" 2> /dev/null; done; rm -rf "$work"' EXIT
# One room of its own and the bots in it: run NAME PLAYERS then the bots' options. Totals to $work/NAME.totals.
run() {
  local name="$1" n="$2"; shift 2
  local port=0 q rp
  for q in 1 2 3 4 5 6 7 8; do port=$(( 20000 + RANDOM % 20000 )); curl -s -m 1 -o /dev/null "http://127.0.0.1:$port/" 2> /dev/null || break; done
  OA_ROOM_TICKET_KEY="$key" "$work/room" --port "$port" --dev "$root/coax/web/room/dev" > "$work/$name.room.log" 2>&1 & rp=$!
  for q in $(seq 1 100); do curl -fs "http://127.0.0.1:$port/health" > /dev/null 2>&1 && break; read -r -t 0.1 < /dev/zero || true; done
  OA_ROOM_TICKET_KEY="$key" coax/web/tool.sh coax/web/room/bots.coax run "ws://localhost:$port" "$n" "$secs" --direct --zone --realm "pair-e2-loop-$name" --reconnect 0 --travel 0 "$@" > "$work/$name.bots.log" 2>&1
  kill "$rp" 2> /dev/null
  grep '^bots ' "$work/$name.bots.log" | tail -1 > "$work/$name.totals"
  [[ -s "$work/$name.totals" ]] || tail -20 "$work/$name.bots.log" > "$work/$name.fail"
}
run party "$players" --quest & pids="$pids $!"
run fallen 1 --class 11 --patrol 330 --meek & pids="$pids $!"
if [[ "$full" == full ]]; then for c in $classes; do run "solo-$c" 1 --quest --class "$c" & pids="$pids $!"; done; fi
for p in $pids; do wait "$p"; done
pids=""
grep -m 1 -i "content" "$work/party.room.log" | sed 's/^/room: /'
num() { grep -o "\"$2\":[0-9]*" "$work/$1.totals" | head -1 | cut -d: -f2; }
missing=""
need() { local name="$1" k v; shift; for k in "$@"; do v="$(num "$name" "$k")"; [[ -n "$v" && "$v" -gt 0 ]] || missing="$missing $name:$k"; done; }
for name in party fallen $( [[ "$full" != full ]] || for c in $classes; do echo "solo-$c"; done ); do
  if [[ -s "$work/$name.fail" ]]; then cat "$work/$name.fail"; echo "loop-proof: the $name bots printed no totals"; exit 1; fi
  echo "$name: $(cat "$work/$name.totals")"
done
want="seated says quest_stages kills loot items"
[[ "$full" != full ]] || want="$want quests_paid coin worn"
need party $want
# a reward is paid once a quest: never more quests paid than bots times the tables' quests
quests="$(sed -n '/^table quest /,/^table /p' everglade/content/everglade.content | grep -c '^[0-9]')"
paid="$(num party quests_paid)"; [[ -z "$paid" || "$paid" -le $(( players * quests )) ]] || missing="$missing party:paid-twice"
# the fallen player: down, and woken whole where it first stood by the room
need fallen seated downs wakes
if [[ "$full" == full ]]; then
  for c in $classes; do
    name="solo-$c"; need "$name" seated kills
    paid="$(num "$name" quests_paid)"; level="$(num "$name" level)"
    echo "solo ${names[$c]}: level ${level:-1}, quests paid ${paid:-0} of $quests, downs $(num "$name" downs), wakes $(num "$name" wakes)"
    [[ -n "$paid" && "$paid" -ge 2 ]] || missing="$missing $name:two-quests-paid"
    [[ -n "$level" && "$level" -ge 3 ]] || missing="$missing $name:level-3"
    [[ -z "$paid" || "$paid" -le "$quests" ]] || missing="$missing $name:paid-twice"
  done
fi
[[ -z "$missing" ]] || { echo "loop-proof: missing:$missing"; exit 1; }
echo "loop-proof: holds ($players bots together, one fallen and woken, $secs s${full:+, the hand-in and each class alone too})"
