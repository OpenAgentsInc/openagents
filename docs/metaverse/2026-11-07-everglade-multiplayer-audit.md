History: see everglade/ARCHITECTURE.md.

# Everglade multiplayer audit: Everglade II as one multiplayer zone

Date: 2026-11-07 (measured 2026-11-08, Chicago time)

This audit covers multiplayer as it runs today, what Everglade II needs so the whole journey is
multiplayer, measurements from a local room, the risks, and an ordered plan. Everglade II
(`/worlds/everglade2`, soon Everglade) runs from Character Create's studio, down one path through
the woods, to a clearing. The goal: from the moment you create a character, you share one room
with everyone else. While the cinematic run walks you to Everglade you see the other new players
running too. Each player has a name they chose, drawn over their head in the UI's font. Everyone
moves smoothly, in the clearing and anywhere else.

The issues that carry the plan are listed at the end.

## What runs today

### The room protocol

The page and the room speak the Everglade wire (`apps/openagents-web/everglade-wire.coax`, version
5) over one WebSocket.

- The page sends a hello (`H`: look, class, flags, name), an input for every 1/30 s step (`I`: sequence,
  keys, sticks, heading, buttons, the newest state heard), casts (`C`) and clock pings (`t`).
- The room sends your seat (`y`), the world's clock (`W`), state frames (`D`) and refusals (`r`).
  A `D` frame holds rows (`s`, deltas against a row the page acknowledged), who each seat is (`n`:
  player id, look, class, flags, name) and removals (`x`).
- Before the hello the page sends its build (`WirePins`); the room refuses a build it doesn't
  run (`r 0 version`) and the page reloads once.

### Authority, prediction and reconciliation

The room is server-authoritative. It steps every seat at 30 Hz from that seat's inputs
(`coax/web/room/room-everglade.coax`, `room-tick.c`, `room-world.c`). For Everglade, `room-world.c`
runs a headless copy of the page's world, so the room's step is the page's step.

The page predicts. The v1 client (`apps/openagents-web/old/v1/everglade-net.coax`, `everglade5e-face.c`):

1. steps you at the room's rate (1/30 s), sending each step's input as the wire carries it;
2. when a `D` row for your seat acknowledges a newer input, sets you where the room has you and
   re-runs the inputs the room hasn't stepped yet (`net_fix`, `net_replay_*`);
3. eases out the difference over `NET_SMOOTH` seconds, or takes it at once past `NET_SNAP`
   (1.5 m).

### Interpolation

Other entities are drawn a jitter buffer behind the server clock. Each entity keeps its last
`NET_RING` (8) states. The buffer's delay follows how late and how spread the states arrive
(100 to 300 ms) and slews there instead of jumping. Past the newest state an entity goes on by its
velocity for a while, then holds. A stop seen late is eased out, never jumped back.

### Join, leave and persistence

- The page asks the API for a seat ticket (`POST /api/playtest/room-ticket/ROOM?realm=…&build=…`).
  The API's directory picks the instance and the shard. The ticket carries your player id and
  name; the room takes the name from the ticket and never from the hello.
- A seat is held 15 s after its socket drops (`LIFECYCLE_HOLD_MS`), so a reload or a network blip
  rejoins the same body.
- Names are `verse.player_name`: an account's name, else the guest's own (`POST
  /api/playtest/player/name`, filtered by `verse.name_ok`: 3 to 20 characters, letters, digits,
  spaces, apostrophes and hyphens, no URLs, a blocked-words list), else two words made from the
  player id.
- Character Create keeps your appearance as a CC3 record: 57 printable bytes
  (`metaverse-engine/character/character-appearance.coax`).

### Production hosting and capacity

- Cloud Run `oa-room` and `oa-room-2` to `oa-room-6`: six shards, routed as `/api/room/sK/`.
- Each process simulates up to 16 rooms of 16 seats (`EG_ROOMS`, `EG_SEATS`).
- A release drains each shard first (`r 0 restarting`); pages rejoin on another shard. A new shard
  takes traffic only after its startup probe (`/ready`) passes.

### Reconnects and offline

A dropped socket comes back on a doubling, jittered wait (1 s to 60 s), each time with a fresh
ticket. A quiet socket (nothing heard for `NET_SILENT_MS`) is closed and reopened. With
`?offline=worker` the page plays the room's own simulation in the browser (`room-worker.c`), which
speaks the same frames.

## What Everglade II lacked

At the start of this audit Everglade II was single-player. It had no socket. It stepped the
controller once per frame with a variable `dt`. Its colliders came from its own planting. If a page
had asked the room for an `everglade2` room, the room would have run Everglade's town world there:
the wrong land, a river and a watch.

| Need | Status at the start | Gap |
|---|---|---|
| A room whose world is Everglade II's land, path and woods | None | The room ran v1's world for every Everglade room |
| Multiplayer from character creation | None | No socket, no ticket, no hello |
| Others visible during the cinematic run | None | Nothing to draw them with |
| Name selection | Guests got a two-word default | No name field in Character Create |
| Nameplates in the UI font | None | Everglade II drew one body, yours |
| Smooth remote movement | None | No interpolation |
| Remote appearance | None | v1 sends `ranger.S.D.T` looks; the room kept 32 bytes of look, too few for a 57-byte CC3 record |
| Town and clearing interest, hotbar and casts | None | The room's interest is per room; casts aren't in Everglade II yet |
| Reliability: reconnect, crash, deploys, load | v1 has them | None of it reached Everglade II |

## Decisions

- **One authority.** Everglade II's room is server-authoritative, built directly; no host-player
  stage. The room picks Everglade II's world by the room code (`everglade2-N` and checks'
  `pair-e2-*`, `room-everglade.coax eg_e2_code`).
- **One movement world.** Where you may stand, the trunk and rock colliders and the pace move into
  `apps/openagents-web/everglade2-move.coax`, which the page and the room both load. The room's
  step (`room-world.c world2_step`) is the page's step (`everglade2-face.c step_frame`), so
  prediction agrees.
- **One client core.** The realm-neutral half of v1's client (the state ring, the jitter buffer,
  the clock, the API's answers) moves into `apps/openagents-web/everglade-net-core.coax`. v1 and
  Everglade II load it; a fix lands once.
- **Join at the run.** The page asks for its ticket during the studio and opens the socket when you
  press USE IN EVERGLADE (or at once, when you arrive with a character). The run holds your feet
  for up to 4 s until you're seated, so the room and the page start you at the same spot.
- **The look is the CC3 record.** The hello's look carries the record; the room keeps 64 bytes of
  it (`EG_LOOK`), and each page draws the others from their records.
- **Names aren't unique.** Identity is the player id. Two players may share a name, as in most MMOs
  with a server-side filter. The name goes through `verse.name_ok` and the blocked-words list on the
  API. Uniqueness would need a reservation table and a rename flow for collisions, and blocks
  nothing a filter doesn't.
- **Nameplates** are drawn with `cc.text` (AR Kai C, Character Create's face) at the projected
  head. They're whole within 20 m, fade out by 32 m, and hide when the land stands between your eye
  and the head.
- **Staged shots stay offline.** `?at=` and `?walk` (and `?film`) open no room.

## Measurements

All numbers are from this Mac (Apple Silicon), one local room server (`coax/web/room/local.coax`,
the room binary built from this change), loopback, on 2026-11-08.

### Bots against the room (`load.coax everglade2`)

`load.coax` gained an `everglade2` mode: each bot joins `pair-e2-load-STAMP-K`, sends a hello, an
input every 1/30 s (a turning walk), a cast every 5 s and a ping a second.

| Run | Input to ACK p50 / p95 / p99 | Ping p50 / p95 | State rate | State gap p99 / max | Gaps over 100 ms | Bytes a second |
|---|---|---|---|---|---|---|
| Everglade II, 3 bots, 20 s | 21 / 28 / 29 ms | 1 / 2 ms | 30 Hz | 35 / 41 ms | 0 | 3 018 |
| Everglade (v1), 3 bots, 20 s | 11 / 19 / 19 ms | 1 / 2 ms | 30 Hz | 35 / 45 ms | 0 | 11 346 |
| Everglade II, 16 bots, 30 s | 19 / 33 / 34 ms | 0 / 2 ms | 30 Hz | 36 / 45 ms | 0 | 13 937 |
| Everglade II, 3 bots, 30 s, 100 ms RTT, ±10 ms jitter, 1% loss (`net-proxy.coax`) | 124 / 217 / 303 ms | 107 / 178 ms | 29.9 Hz | 221 / 253 ms | 29 | 3 053 |

The ACK time on loopback is the wait for the room's next tick. Everglade II sends a quarter of
v1's bytes: its room has no town, river or watch. Through the bad network the longest state gap
(253 ms) stays inside the jitter buffer's ceiling (300 ms). The casts are refused in Everglade II's
room today (the load's `refused` count); casts are issue F's.

### Three headless Chromes through the whole journey (`everglade2-crowd`)

`coax/web/tool.sh everglade2-crowd URL --players 3` opens Character Create in three headless
Chromes, presses USE IN EVERGLADE in each, and watches the run and a walk after it.

| What | Result |
|---|---|
| Seated after USE IN EVERGLADE | 0.6 to 1.5 s, while the run plays |
| Others drawn during the run | All three pages drew both others |
| The first other's drawn speed during the run | 4.03 to 4.06 m/s (the run, curving down the path) |
| Stalls and lurches of the first other, frame to frame | 0 and 0 over about 1 300 moving frames a page |
| Your own re-runs' largest move | 11 to 40 mm during the run |
| Every other's name on every page | Yes (two each) |

One defect showed here and is fixed. The page kept you where you may stand measured from where
you were last drawn, while the room measured from where the step began. Walking into the clearing's
edge, the room and the page disagreed by up to 2.4 m a re-run. The page now keeps you from the
step's start, as the room does (`e2_keep_from`); the largest re-run fell to 130 mm.

### Qualification, 2026-11-08

All of this ran on this Mac against a local room, in headless Chrome with the scene drawing. Real Chrome, a phone and the staging path are still to do.

**The journey and names (3 pages).** Each page presses SURPRISE ME, so each is someone different, and takes a name in NAME. The first page's "http x" is said and not sent. Every page draws the other two during the run and has their chosen names when it first draws them. Names show over heads (as many as 2 in a frame). The others run at 4.0–4.1 m/s with 0 stalls. Re-runs are at most 44 mm.

**13 others in the room** (2 pages among 12 bots):
- 60 FPS: frame gap p50 16.7 ms, p99 17.6–18.6 ms, 0 frames over 25 ms.
- Coax frame p50 1.1–1.3 ms, p99 1.8 ms.

**Soaks:**
- 12 bots for 11 minutes, with 2 pages joining partway: 0 state frames dropped, no ACK going back, state gap p99 35 ms (max 61 ms), 0 stalls. Room memory went from 203.7 MB to 204.9 MB.
- 16 bots for 10 minutes, beside a second room of 14 bots and 2 pages (16 players): 0 dropped, no ACK going back, gap p99 39 ms, 14.2 kB/s. There were 70 stalls of 100 ms or more (max 218 ms) while the Mac was loaded by other builds; the 12-bot soak on a quieter machine had none. Memory went from 203.2 MB to 205.2 MB.

**Simulated network.** Through `net-proxy.coax` at 100 ms, 20 ms jitter, 1% loss:
- The others: 0–3 one-frame stalls in about 1,100 moving frames per page.
- Re-runs at most 43 mm.

At 300 ms, 20 ms jitter, 1% loss:
- The others: 0–1 stalls.
- Re-runs at most 92 mm.

**A room killed mid-walk and started again.** Every page rejoined and drew the others again. Without a checkpoint store (none locally), the restarted room put everyone back at the spawn, a 127 m snap. Issue E carries this.

**Still to do:**
- a drain to another shard;
- position across a room restart;
- the staging path with real Chrome on two machines, and a phone;
- casts and interest along the path (F).

## Risks

- **Prediction drift at colliders.** Any difference between the page's and the room's step shows
  as a re-run. The keep fix above was one. Both now run one module; a check that steps the page's
  and the room's code on the same inputs and compares them would catch the next.
- **Draw budget.** Each body is up to 11 draws and records. Sixteen bodies add 176 of each, so
  `E2_RECORDS` rose to 256 and `E2_DRAWS` to 640. Frame time with 10 or more remotes is unmeasured.
- **The pack in headless checks.** The crowd check proves the room and the net; it can't prove
  drawing until it runs against a server with the host modules.
- **Room size.** A room seats 16. A popular launch fills rooms of 16 along the path; the directory
  spreads players over instances, so friends may not see each other unless they share a party
  or an invite.
- **Names.** Non-unique names can be impersonated; the player id behind a name is what BLOCK and
  REPORT act on.
- **Headless isn't proof.** Every result above is headless. Real Chrome on a real network must
  pass before release.

## Plan

In order:

1. **A. The room's Everglade II world.** The room chooses it by code and steps each seat on
   Everglade II's land with the shared movement module. Done with this audit.
2. **B. Everglade II's client net.** The shared core, the ticket, the hello with your record,
   fixed-step prediction and re-runs, remote interpolation, the run holding for its seat,
   reconnects. Done with this audit; real-Chrome checks remain.
3. **C. Remote characters.** Each other player drawn with their own record, gait, jumps and facing;
   frame time with 10 or more remotes measured at 60 FPS.
4. **D. Names.** A name field in Character Create (POST `player/name`, then a new ticket so the
   room has it), and nameplates in AR Kai C with fade and occlusion.
5. **E. Reliability qualification.** Three or more headless clients through the whole journey,
   a soak, loss and latency through `net-proxy.coax`, a room crash and a drain, the staging path,
   and frame time with 10 or more remotes.
6. **F. Casts and interest.** The class hotbar's casts through the room, and interest management
   as the zone grows past one room's reach.

## Issues

| Step | Issue |
|---|---|
| A. The room's Everglade II world | [#13053](https://github.com/OpenAgentsInc/openagents/issues/13053) |
| B. Everglade II's client net | [#13054](https://github.com/OpenAgentsInc/openagents/issues/13054) |
| C. Remote characters, 10+ at 60 FPS | [#13055](https://github.com/OpenAgentsInc/openagents/issues/13055) |
| D. Names and nameplates | [#13056](https://github.com/OpenAgentsInc/openagents/issues/13056) |
| E. Reliability qualification | [#13057](https://github.com/OpenAgentsInc/openagents/issues/13057) |
| F. Casts and interest | [#13058](https://github.com/OpenAgentsInc/openagents/issues/13058) |
