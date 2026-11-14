#!/usr/bin/env bash
# Firm reason: 5 (thin wrapper: builds and starts the room server, then runs the app against it; holds no logic)
# Runs the Everglade desktop app against a room on this Mac of the same checkout, as coax/web/serve.sh does
# for the web (#13589): coax/web/room/bin/room --dev answers the seat tickets and the room's socket, and
# the app (a development build, apps/everglade-desktop/build.sh) is told where with EVERGLADE_ROOM_ORIGIN.
# The site's room plays only what production runs; this one plays this checkout (the range's Longbow, its
# arrows). The room stops when the app quits.
#   apps/everglade-desktop/local-room.sh [APP ARGS...]
#     no arguments: opens the app (built by build.sh) and waits until you quit it
#     --scene 3: opens it at the Archery Range
#     --shot OUT.png --scene 3 --online ...: a headless shot against the local room (--online is needed)
set -Eeuo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
app="$here/build/Everglade.app"
[[ -x "$app/Contents/MacOS/Everglade" ]] || { echo "local-room: build the app first: apps/everglade-desktop/build.sh" >&2; exit 1; }
room_bin="$repo/coax/web/room/bin/room"
# Built when missing or older than a source, as serve.sh does.
if [[ ! -x "$room_bin" || -n "$(find "$repo/coax/web/room" "$repo/everglade" "$repo/metaverse-engine" -name '*.coax' -newer "$room_bin" -not -path '*/build/*' -print -quit 2>/dev/null)" ]]; then
  echo "local-room: building the room server"
  "$repo/coax/web/room/build.sh" > /dev/null
fi
port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')"
log="${TMPDIR:-/tmp}/everglade-local-room-$port.log"
"$room_bin" --listen 127.0.0.1 --port "$port" --dev "$repo/coax/web/room/dev" > "$log" 2>&1 & room_pid=$!
trap 'kill "$room_pid" 2>/dev/null || true' EXIT
for _ in $(seq 1 50); do grep -q "listening on" "$log" 2>/dev/null && break; sleep 0.1; done
echo "local-room: the room answers on 127.0.0.1:$port (pid $room_pid, its log: $log)"
export EVERGLADE_ROOM_ORIGIN="http://127.0.0.1:$port"
if [[ " $* " == *" --shot "* ]]; then
  "$app/Contents/MacOS/Everglade" "$@"
else
  open -W --env EVERGLADE_ROOM_ORIGIN="$EVERGLADE_ROOM_ORIGIN" "$app" ${1+--args "$@"}
fi
