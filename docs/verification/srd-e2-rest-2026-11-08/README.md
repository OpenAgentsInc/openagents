# Everglade II rest belongs to the room, through reconnects and restarts

Partial #13046 work: 9a8d8b6250 (the room's rest and the hotbar controls),
then the rest carry and checkpoint profile 3. Cached compiler 0.52.0.

## What changed

- The Everglade II room enables the shared rest scheduler and is the only
  authority over a seat's rest. A page asks with `Z SEQ REQ`; the room admits
  it through `rs_rest_control` only after its own accepted steps show the seat
  placed, grounded, still, with released controls, and every queued input
  released too (a sleeping body's queued keys don't count: they move nothing).
  Walking and jumping come from accepted steps and real displacement. A
  sleeping seat's steps carry no locomotion. The room tells the seat
  `z SEQ CODE DICE` on every change and on a grace rejoin.
- The hotbar shows rest buttons from the room's state (Short Rest, Long Rest,
  Watch, Sleep, Hit Die, End) and the health line says SHORT REST, HIT DICE N,
  ASLEEP, ON WATCH, ON THE MOVE or REST BROKEN. A refusal shows "YOU CAN'T REST
  RIGHT NOW" for four seconds. No banners.
- `aethyr/rules/srd/rest-carry.coax` turns a creature's rest into 19 bounded
  integers and lays them back on a fresh body all or none, with no offline
  credit and the 16-hour Long Rest gate kept.
- Character checkpoint profile 3 (`docs/protocol/character-checkpoint-v3.md`)
  is profile 2 plus that rest. An Everglade II room captures it and restores
  it after a kill or drain. The page asks `checkpoint=2&rest=1`; otherwise the
  API and the dev store give the profile 2 part. A room whose profile 3 post is
  refused (400 or 5xx) falls back to profile 2. Migration 0136.

## Checks

```
coax/bin/coax test coax/web/room/room.coax --only 'e2 rest'                 # 6 passed
coax/bin/coax test coax/web/room/room.coax                                  # 1007 of 1009; the 2 room raid failures reproduce on unchanged origin/main
coax/bin/coax test aethyr/rules/srd/short-rest-checks.coax --library --only 'rest carry'   # 5 passed
coax/bin/coax mutate aethyr/rules/srd/short-rest-checks.coax --library --unit aethyr/rules/srd/rest-carry.coax   # 279 killed, 0 survived
coax/bin/coax test aethyr/session/character-checkpoint.coax                 # 81 passed
coax/bin/coax mutate aethyr/session/character-checkpoint.coax --root-only   # 248 killed, 0 survived
API_TESTS_ONLY=checkpoint2_checks.coax coax/web/api/test.sh                 # 26 checks incl. 4 resume3, real scratch Postgres with 0136
coax/bin/coax test apps/openagents-web/everglade2.coax --only rest          # 57 passed (and 'Everglade II' 11, 'ticket' 3)
coax/web/tool.sh tools-check
coax/web/wasm.sh apps/openagents-web/everglade2.coax ...                    # 641,847 bytes, under the unchanged 660,000 budget
```

Real sockets: `coax/web/tool.sh coax/web/room/e2-restart-check.coax --rest`
runs two native WebSocket players against the real room binary (`--dev`, its
own temporary store, an ephemeral loopback port). Player 0 settles and sleeps
(z 3), walks while asleep without moving, takes the watch (z 4), walks (z 5)
and stops (z 4). Player 1's Short Rest (z 1) breaks on walking (z 0). Player 0's
socket drops and sits again within the grace and hears its watch. It sleeps,
the room is SIGKILLed after a checkpoint and started again on its store, and
player 0 sits again asleep (z 3) and still can't move; player 1 has no rest.
See "Socket runs" below.

The first socket runs found a real defect: the room refused a rest whenever an
input was still queued, and a page sends one every 1/30 s, so a request was
refused or admitted by chance. Admission now looks at the queued inputs.

The room binary for the socket runs was built from this tree with the two
failing `room raid` asserts disabled locally (never committed): `build.sh`
runs every room check, and those two fail on unchanged origin/main.

## Limits

No headless browser run of the rest buttons. The checks cover the room, wire,
page state and drawing functions. Rolling the API back after it stored profile
3 bodies hands them to pages as they are, so roll back rooms and pages first.

## Socket runs

After the queued-input fix, four runs in a row hold, every step ok
(`socket-rest.log` is the first). Before it, runs failed at the first rest
request by chance, which is how the defect showed.
