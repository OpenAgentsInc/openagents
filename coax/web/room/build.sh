#!/usr/bin/env bash
# Builds the Orbit League room server (room.coax, room-host.c) to OUT
# (default coax/web/room/bin/room).
#   coax/web/room/build.sh [OUT]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
root="$(cd "$here/../../.." && pwd -P)"
out="${1:-$here/bin/room}"
mkdir -p "$(dirname "$out")"
work="$(mktemp -d "${TMPDIR:-/tmp}/coax-room-build.XXXXXX")"
trap 'rm -rf "$work"' EXIT
"${COAX:-$root/coax/bin/coax}" c "$here/room.coax" --library --header "$work/room.h" > "$work/room.c"
cc -std=c11 -O2 -w -c "$work/room.c" -o "$work/room.o"
cc -std=c11 -O2 -Wall -Wextra -D_DEFAULT_SOURCE -I"$work" -I"$root/verse/voice-host" "$here/room-host.c" "$root/verse/voice-host/ws.c" \
  "$work/room.o" -lm -o "$out"
echo "built $out"
