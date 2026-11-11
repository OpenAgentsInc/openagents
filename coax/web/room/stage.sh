#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: turns the room's Coax units into C and gathers its C sources)
# Stages the room server's sources in OUT: OUT/src holds the Coax units turned into C (room.c and
# the everglade world's everglade.c, everglade-step.c, grid.c, river.c, guard-guide-policy.c), the
# host C (room-host.c and the room-tick.c it includes, room-world.c, room-math.c, ws.c) and the
# catalog (room-catalog.c); OUT/compat is coax/web/compat. compile.sh turns OUT/src into the
# `room` binary. Cloud Run's image (deploy.sh, Dockerfile) and the VM (vm-deploy.sh) both start here.
#   coax/web/room/stage.sh OUT
# Coax to C runs every check the room's units keep: run it on a build host,
#   coax/remote/remote.sh run --no-fallback -- coax/web/room/stage.sh coax/web/room/build/stage
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
out="${1:?usage: coax/web/room/stage.sh OUT}"
mkdir -p "$out"; out="$(cd "$out" && pwd)"
cd "$root"
rm -rf "${out:?}/src" "${out:?}/compat"
mkdir -p "$out/src"
coax="${COAX:-coax/bin/coax}"
"$coax" c coax/web/room/room.coax --library --header "$out/src/room.h" > "$out/src/room.c"
# the everglade rooms' world (room-world.c): the city's units, as build.sh links them
for u in apps/openagents-web/everglade.coax apps/openagents-web/everglade-step.coax verse/grid.coax verse/everglade/river.coax verse/everglade/guard-guide-policy.coax; do
  n="$(basename "$u" .coax)"
  "$coax" c "$u" --library --header "$out/src/$n.h" > "$out/src/$n.c"
done
cp "$here/room-host.c" "$here/room-tick.c" "$here/room-stats.h" "$here/room-world.c" "$here/room-world.h" "$here/room-math.c" verse/voice-host/ws.c verse/voice-host/ws.h "$here/compile.sh" "$out/src/"
coax/web/tool.sh "$here/room-catalog.coax" apps/openagents-web/realms/everglade/assets/everglade.pack "$out/src/room-catalog.c"
cp -R coax/web/compat "$out/compat"
git rev-parse HEAD > "$out/commit" 2>/dev/null || true
echo "room: staged in $out"
