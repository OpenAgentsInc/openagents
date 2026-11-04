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
coax="${COAX:-$root/coax/bin/coax}"
"$coax" c "$here/room.coax" --library --header "$work/room.h" > "$work/room.c"
# Everglade's headless world for the everglade rooms (room-world.c): the city's unit (everglade.coax,
# nothing drawn), the controller's (everglade-step.coax, with aethyr/player.coax) and the units the
# city links beside them, as apps/openagents-web/build.sh links the page's.
units=(apps/openagents-web/everglade.coax apps/openagents-web/everglade-step.coax verse/grid.coax verse/everglade/river.coax verse/everglade/guard-guide-policy.coax)
for u in "${units[@]}"; do
  n="$(basename "$u" .coax)"
  (cd "$root" && "$coax" c "$u" --library --header "$work/$n.h" > "$work/$n.c")
done
objs=("$work/room.o")
cc -std=c11 -O2 -w -c "$work/room.c" -o "$work/room.o" &
for u in "${units[@]}"; do
  n="$(basename "$u" .coax)"
  # at -O2, as the image builds them (Dockerfile): the tick runs them for every room 30 times a second
  cc -std=c11 -O2 -w -c "$work/$n.c" -o "$work/$n.o" &
  objs+=("$work/$n.o")
done
wait
# The room's unit and the city's both carry apps/openagents-web/analytics.coax's exports (Orbit League
# loads it too): the room's copies become local to it, so the city's are the program's.
nm -g --defined-only "$work/room.o" | awk '{print $3}' | sort > "$work/room.syms"
nm -g --defined-only "$work/everglade.o" | awk '{print $3}' | sort > "$work/city.syms"
comm -12 "$work/room.syms" "$work/city.syms" > "$work/both.syms"
if [[ -s "$work/both.syms" ]]; then
  if [[ "$(uname)" == Darwin ]]; then
    ld -r "$work/room.o" -o "$work/room-own.o" -unexported_symbols_list "$work/both.syms"
  else
    objcopy --localize-symbols="$work/both.syms" "$work/room.o" "$work/room-own.o"
  fi
  objs[0]="$work/room-own.o"
fi
cc -std=c11 -O2 -w -I"$work" -c "$here/room-world.c" -o "$work/room-world.o"
# the pack's catalog, the bounds the world's pieces break with (room-catalog.py)
python3 "$here/room-catalog.py" "$root/apps/openagents-web/realms/everglade/assets/everglade.pack" "$work/room-catalog.c"
cc -std=c11 -O2 -w -c "$work/room-catalog.c" -o "$work/room-catalog.o"
cc -std=c11 -O2 -Wall -Wextra -D_DEFAULT_SOURCE -I"$work" -I"$here" -I"$root/verse/voice-host" "$here/room-host.c" "$root/verse/voice-host/ws.c" \
  "$work/room-world.o" "$work/room-catalog.o" "${objs[@]}" -lm -o "$out"
# the started server holds every declared default (room-host.c --start-check, room.coax rm_start_check)
"$out" --start-check
echo "built $out"
