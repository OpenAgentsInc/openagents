#!/usr/bin/env bash
# A local Everglade room as production has it: the room server and the page server, with the room
# as the API's stand-in (room-host.c --dev, coax/web/room/dev/). The page asks its usual /api/ paths,
# which serve.sh --api sends to the room, so the page and the room read the same world document
# (dev/world-everglade-5e.json) and the same seat tickets, names, levels and cap (dev/room-control.txt).
#   coax/web/room/local.sh [--room PORT] [--page PORT] [--static] [--no-build]
# Prints the page's URL (ROOM= and PAGE= lines for scripts), then runs until stopped; both servers stop with it.
# --static serves the last build without watching (as the harnesses run it); --no-build skips the room build.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
room_port=8841; page_port=8875; static=(); build=1
while [[ $# -gt 0 ]]; do
  case "$1" in
    --room) room_port="$2"; shift 2 ;;
    --page) page_port="$2"; shift 2 ;;
    --static) static=(--static --cache 0); shift ;;
    --no-build) build=0; shift ;;
    *) echo "usage: local.sh [--room PORT] [--page PORT] [--static] [--no-build]" >&2; exit 2 ;;
  esac
done
for p in "$room_port" "$page_port"; do
  if [[ "$p" == 8830 || "$p" == 8872 ]]; then echo "local.sh: $p is the owner's; pick another port" >&2; exit 2; fi
done
if [[ "$build" == 1 || ! -x "$here/bin/room" ]]; then "$here/build.sh"; fi
pids=()
trap 'for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done' EXIT INT TERM
"$here/bin/room" --port "$room_port" --dev "$here/dev" & pids+=($!)
"$root/coax/web/serve.sh" "$root/apps/openagents-web" --listen "127.0.0.1:$page_port" --api "$room_port" ${static[@]+"${static[@]}"} & pids+=($!)
echo "ROOM=ws://localhost:$room_port"
echo "PAGE=http://localhost:$page_port"
echo "open http://localhost:$page_port/worlds/everglade?relay=ws://localhost:$room_port"
wait
