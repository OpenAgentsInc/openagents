#!/usr/bin/env bash
# Firm reason: 5 (a thin wrapper: turns the room's Coax units into C and gathers its C sources)
# Stages the room server's sources in OUT: OUT/src holds the Coax units turned into C (room.c and
# the controller's everglade-step.c), the
# host C (room-host.c and the room-tick.c and room-wt.c it includes, room-world.c, room-math.c, ws.c,
# WebTransport's wt.c and the vendored QUIC stack in third_party/); OUT/compat is coax/web/compat. compile.sh turns OUT/src into the
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
# the controller's unit beside the room's (room-world.c), as build.sh links it (#13267)
for u in apps/openagents-web/everglade-step.coax; do
  n="$(basename "$u" .coax)"
  "$coax" c "$u" --library --header "$out/src/$n.h" > "$out/src/$n.c"
done
cp "$here/room-host.c" "$here/room-tick.c" "$here/room-stats.h" "$here/room-world.c" "$here/room-world.h" "$here/room-math.c" verse/voice-host/ws.c verse/voice-host/ws.h "$here/compile.sh" "$out/src/"
cp "$here/room-wt.c" "$here/wt.c" "$here/wt.h" "$out/src/"; cp -R "$here/third_party" "$out/src/third_party"   # WebTransport (#13176)
cp -R coax/web/compat "$out/compat"
git rev-parse HEAD > "$out/commit" 2>/dev/null || true
echo "room: staged in $out"
