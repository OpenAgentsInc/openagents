# Actual local room sockets

Partial #13046 evidence on 7a9b3c254a. Reused the previously checked native
host `/tmp/srd-room-traversal-server`; no production source changed. It ran
only on 127.0.0.1:18983 with the repository's explicitly public development
fixture key, dev world and a separate `/tmp` dev store. No owner's persistent
files or running service were used. The owned server was gracefully stopped
and its session exited 0 after all clients finished. No credentials/store
contents are committed.

## HP persists through actual close/rejoin

The existing native TLS/socket-capable Coax harness opens actual WebSockets,
provokes the watch, closes its player connection while another keeps the room
alive, and rejoins with a fresh ticket for the same player:

- Hurt: HP starts 57, leaves at 53, returns at 37, and is 32 after another two
  seconds of the live watch. Rejoining grants no heal; bad decoded rows: 0.
- Down: starts 57, leaves at 0, returns at 0, remains 0 two seconds later;
  bad rows: 0.

Both runs print `keeps`, exit 0. These exercise actual sockets/native room
health persistence, not only interpreter room scenarios.

## E2 harness correction and real casts

The preexisting interest harness sent legacy slot 1 and a legacy Druid hello
while calling its mode "casting". It did not inspect cast outcomes, so interest
success could never establish cast success. It now sends `srd-druid` and the
published class-kit Druid first-slot wire identity (200: 128 + 3 * 24), parses
own/peer cast events and matching refusals, and requires accepted casts from
each near caster, no refusals and peer events. The numeric slot is an
independent wire fixture; actual server acceptance validates it. A new codec/
receipt regression was authored before implementation; all 258 loaded checks
pass. No core admission policy or mutation gate changed. Tools-check passes.

The corrected 90-second actual-socket run with four clients/two near passes:
36 sent, 36 accepted, zero refused, 72 peer cast events; 2,108 near pairs held,
zero missed; 1,376 far pairs culled, zero retained; 12 arrivals and eight exits.
All original interest assertions remain. The initial ten-second run is retained
as a failed diagnostic: it was too short to exercise far culling/exits and did
not provide cast receipt evidence. Its failure is not suppressed or counted
as a pass. A separate overlong-code invocation failed dev ticket admission;
using a code within the existing 24-byte limit corrected that input, without
changing the server or its admission policy.

Commands (with the public development fixture environment configured):

```
/tmp/srd-room-traversal-server --listen 127.0.0.1 --port 18983 --dev coax/web/room/dev
coax/web/tool.sh coax/web/room/hp-rejoin-check.coax ws://127.0.0.1:18983 90 hurt
coax/web/tool.sh coax/web/room/hp-rejoin-check.coax ws://127.0.0.1:18983 90 down
coax/web/tool.sh coax/web/room/e2-restart-check.coax --interest --players 4 --near 2 --seconds 90 --external 18983 --code pair-e2-srd-verified
coax/bin/coax test coax/web/room/e2-restart-check.coax --library
```

These do not qualify rendered browsers, audible output, every class/spell,
all levels, network-loss/restart behavior or the complete #13046 migration.
The 90-second interest sample is not a production scale/performance benchmark.
The harness's restart/drain modes were not rerun merely because its interest
cast path changed; no new restart/drain claim is made.
