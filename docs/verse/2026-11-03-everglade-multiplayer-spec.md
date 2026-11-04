# Everglade multiplayer specification

Date: 2026-11-03. Status: specification, rewritten the same day for the owner's decision that the
server is authoritative from the start (epic #12718). There are no interim phases and no room host
on a player's page. Section 13 lists what is built and what is not, item by item.

This spec makes Everglade a shared world. Anyone, guests included, opens
`https://openagents.com/worlds/everglade`, gets a character of their own and
runs around, and everyone else in that room sees them move, animate and cast.
**All world state is shared**: the townsfolk, the town watch, creatures that
spells spawn, spell zones, broken buildings and their rebuild, admin edits,
the weather and the time of day. If you spawn a creature, everybody sees it.

Everglade 5E (`/worlds/everglade-5e`, `apps/openagents-web/everglade5e.coax`)
becomes the real Everglade at `/worlds/everglade`. The previous Everglade
module is deprecated (section 12).

Related reading:

- [docs/verse/everglade.md](everglade.md), the realm's design.
- [Aethyr book chapter 49, rooms and presence](../../aethyr/book/ch049-rooms-and-presence.md)
  and [chapter 50, shared state over the network](../../aethyr/book/ch050-shared-state-over-the-network.md).
- [The Verse protocol](openagents-verse-protocol.md), sections 6.3, 10.6
  (`presence.v1`), 10.8 (`realm.v1`) and 10.9 (`sim.v1`).
- [The D&D campaign creator spec](../dnd/2026-11-03-dnd-campaign-creator-spec.md),
  "Persistent world (West Marches style)".

## Contents

1. Goals
2. What exists today
3. Architecture
4. Identity
5. Characters
6. Combat, spells and the shared world
7. Persistence
8. The Grid, the portal and links
9. Scale and cost
10. Security
11. Testing
12. Deprecating the old Everglade
13. Build list and status
14. Open questions for the owner
15. Where the Aethyr book and the protocol are out of date

## 1. Goals

1. A guest or a logged-in player opens `/worlds/everglade` and spawns with
   their own character. No login is needed to play.
2. Everyone in a room sees everyone else: real Ranger bodies (not cones),
   walking, running, jumping, swinging and casting, with their name over them.
3. Remote motion looks smooth and arrives fast: a key press on one screen is
   drawn on another within 250 ms at the 95th percentile on a normal
   connection.
4. All world state is shared. One authority per room simulates the townsfolk,
   the watch, spawned creatures, spell zones and the town's damage, and every
   player sees the same result.
5. A link drops a friend into your room.
6. It runs on our own GCP (the existing `oa-room` Cloud Run service) and adds
   less than $100 a month at expected load. No paid realtime services.
7. Phones keep 60 FPS with a full room.

Non-goals for now: trading, an economy, PvP by default, and voice chat.

## 2. What exists today

### 2.1 How other players are seen in Everglade

- **Transport: HTTP polling, once a second.** The face posts
  `{x, y, z, yaw, after}` to `POST /api/playtest/presence/everglade`
  every `ROOM_EVERY_S` = 1.0 s
  (`apps/openagents-web/everglade-face.c:423`, `room_out` at line 430). The
  body is built by `room_presence_body` (`apps/openagents-web/room.coax:79`).
  `y` is sent only when you are airborne.
- **Server: SQL.** `coax/web/api/room.c` hands the request to `verse.room()`
  (`coax/web/api/migrations/0014_verse_rooms.sql`, later replaced by 0069,
  0074 and 0108). A room holds 16 (`verse.room_max()`), and anyone seen in
  the last 6 s is present (`verse.room_present_ms()`). The answer carries the
  others and the chat lines since `after`.
- **Drawing: the "cone".** `room_draw` (`apps/openagents-web/everglade-room.coax:62`)
  draws each of the nearest 8 others (`ROOM_DRAWN`) within 80 m (`ROOM_NEAR`)
  as four engine shapes: a box (legs), a green cylinder (cloak), a sphere
  (head) and a green **cone** (hood). This is the simple figure the owner saw.
  There is no skinning, no animation clip and no look.
- **Motion: easing, not interpolation.** `room_ease` (`room.coax:242`) moves
  each drawn figure 8/s of the way to the last reported position. With 1 s
  samples a remote player slides in steps and lags 1 to 2 s behind.
- **Names** come from the server: the account's name, else `Guest` and four
  letters of the player id (`0014_verse_rooms.sql`, the `who :=` line).
- **Chat** goes to `POST /api/playtest/chat/<room>`: 500 characters at most,
  5 lines per player in 10 s, and every line kept in `verse.room_chat`.

### 2.2 What Everglade 5E syncs

- Its room is its own, `everglade-5e` (`E5_ROOM`,
  `apps/openagents-web/everglade5e.coax:28`), polled the same way
  (`apps/openagents-web/everglade5e-face.c:487`, 518 to 523).
- It sends each player's HP, maximum HP, condition bits and the last cast's
  name as **hidden chat lines** that start with `[5E] ` (`E5_SYNC`, line 38)
  at most every 2.5 s (`E5_SYNC_EVERY`, line 39; `e5_sync_take`, line 478).
  Other pages read them, keep the numbers and drop the lines from the chat
  panel (`e5_room_read`, line 426).
- Gaps: the lines share the 5-in-10-s chat limit with what the player types,
  they are stored forever in `verse.room_chat`, they carry a cast's name only
  (no aim, no effect), and they arrive 1 to 3.5 s late.

### 2.3 The room server and its relay (WebSocket)

- `coax/web/room/` builds one binary, `room`, from `room.coax` (Coax, compiled
  to C) and `room-host.c` (TCP, the RFC 6455 handshake via
  `verse/voice-host/ws.c`, and a 60 Hz beat). Locally it listens on 8830.
- It serves two things:
  - Orbit League's server-authoritative rooms at
    `/api/room/orbit-league/NAME`. The server steps the ball at 120 Hz
    (`orbitleague.coax` `room_step`) and writes each player the state at
    60 Hz.
  - A **relay** for any game at `/api/room/relay/GAME/CODE`
    (`coax/web/room/room-relay.coax`). It carries each player's text frames
    to the others, unread, and tells them `y SEAT N` (you are in), `j SEAT`
    (joined), `l SEAT` (left) and `m SEAT TEXT` (a frame).
- **Production:** Cloud Run service `oa-room` in `us-central1`, project
  `openagentsgemini`, revision `oa-room-00010-fft` (before #12705), 1 vCPU, 256 MiB,
  min 0 and max 1 instance, concurrency 64, timeout 3600 s, session affinity
  (`coax/web/room/deploy.sh`). The load balancer routes `/api/room/` on
  openagents.com to it. It answers on `wss://openagents.com/api/room/...`
  today.
- Limits that blocked Everglade before #12705 (section 3.3 lists what
  replaced each; production is now `oa-room-00013-vsb` (the simulated everglade rooms, from f9ef1f0c4c), concurrency 1000,
  512 MiB):
  - `RELAY_SEATS` = 4 and `RELAYS` = 16 (`room-relay.coax:13-14`).
  - The seat is written as **one digit** (`rl_head`, line 89, `'0' + seat`),
    so a seat above 9 breaks the format.
  - `CONNS` = 32 in both `room.coax` and `room-host.c:25`.
  - `RELAY_QUEUE` = 48 frames (line 18). A full queue **drops** frames.
  - `send_all` waits up to 50 ms on one slow socket (`room-host.c:44`), so
    one slow phone delays everyone.
  - Cloud Run concurrency 64 caps the instance at 64 WebSockets.
  - A socket is closed after 20 s without a frame (`room-host.c`, the
    `20000` check). Cloud Run closes every socket after 3600 s.
  - The relay has no identity: frames are self-asserted, and nothing ties a
    seat to a player key.
- `coax/web/host.js` gives a page up to 4 WebSockets
  (`coaxial_socket_open`, `_send`, `_close`; events reach the app as
  messages `256 + 4 * id + event`). Everglade's face uses none today.

### 2.4 One-Shot and the lockstep games

- One-Shot (`apps/openagents-web/oneshot.coax:14-16`, 267 to 400) runs over
  the relay at `/api/room/relay/one-shot/CODE`. The **GM's page is the
  authority**: it holds the table and sends the view; players send asks. With
  no relay, everyone plays on the GM's page. `?relay=ws://localhost:PORT`
  points it at a local room server (line 269).
- Belt 2200 and Hold the Stair run in **lockstep** over the relay
  (`apps/openagents-web/lockroom.coax`): orders, commits and state hashes,
  snapshots in base64 chunks under the 4,096-byte frame.
- So the repo already has the three models: server authority (Orbit League),
  one client as authority (One-Shot), and lockstep (Belt 2200).

### 2.5 The Grid

- The Grid (`/worlds/the-grid`, `apps/openagents-web/newland-walk.coax`)
  posts to the playtest room `the-grid` every 0.2 s (`WK_POST_EVERY`,
  line 21) over the same HTTP presence, and the homepage draws those walkers
  (`newland-walkers.coax`).
- The Grid's Everglade gate and portal are art (`gridcrowd.coax`, the crowd
  demo). **No web code hands a player from the Grid into Everglade.** The
  galaxy opens `https://openagents.com/worlds/everglade` by navigation
  (`newland.coax:2269`). The previous Rust engine had a walk-in arch from the
  app's Grid and a shared instance on a chamber host
  (`verse/everglade/previous/everglade.md`, "A shared instance runs on a
  chamber host"); that code is gone.

### 2.6 What runs per page today (local-only)

Everything below runs on each page by itself. Two players in the same room
see different results.

| State | Where | Shared today? |
| --- | --- | --- |
| Your position, yaw, gait | `aethyr/player.coax`, `everglade-face.c` | Yes, 1 Hz, as a cone figure |
| Your animation clip (idle, walk, run, jump, swing) | `figure`, `everglade.coax:2954`; clips in `verse/everglade/ranger.coax:14` | No |
| Townsfolk positions and routines | `verse/everglade/folk.coax`, `routines.coax`; `folk_clock`, `everglade.coax:2707` | Same only by construction: a pure function of the seed and the page's `now`. Clock skew between machines shows as different spots |
| Townsfolk HP, knock-downs, deaths, frogs | `everglade-watch.coax`, `everglade-abilities.coax`, `everglade-circle.coax` | No |
| The watch, the chase, WANTED | `everglade-watch.coax`, `verse/everglade/watch.coax`; [everglade.md](everglade.md) line 789: "Hostility is your page's alone" | No |
| Guard captain and guide decisions | `everglade-guard-ai.coax`, `everglade-guide.coax`, `verse/everglade/captain.coax` | No |
| Your HP, conditions, casts (5E) | `everglade5e.coax`, `everglade5e-rules.coax` | HP, conditions and the cast's name every 2.5 s as hidden chat lines |
| Spell flights and blasts (meteor, bolt, orb) | `SpMeteor`, `SpBlast`, `SpBolt`, `everglade-spells.coax:140-151` | No |
| Lingering zones (frost patches, storms, fire) | `everglade-frost.coax:65-73`, `everglade-storm.coax:47`, `everglade-fire.coax` | No |
| Spawned creatures (Conjure Animals' pack, Polymorph's frog, your Wild Shape) | `ci_ca_at`, `everglade-circle.coax:264`; forms at line 51 | No |
| Broken buildings, debris, rubble, burning | `everglade-spells.coax`, `everglade-rubble.coax:24`, `everglade-fire.coax`; [everglade.md](everglade.md) line 661: "nothing broken is saved or shared" | No |
| R (REBUILD) | `sp_rebuild`, `everglade-spells.coax:478` | No |
| The trees outside the city | `everglade-forest.coax` | No |
| The flock that follows you | `aethyr/flock.coax` | No |
| Weather | `weather_facts`, `everglade.coax:3074`: seeded with `EG_WEATHER_SEED` and the page's `now` | Same only by construction, as the folk |
| Time of day | `folk_clock`; the realm's `clock: cycle` | Same only by construction |
| Admin edits (Aethyrforge, voice edits, placed things) | `everglade-forge.coax`, `everglade-edit.coax`, `verse/edit/edit.coax` | Published to `/api/playtest/world/<id>`. Other pages see them on reload; only an owner's page rereads the spec every second (`verse/edit/edit.coax:589`) |
| Dropped items | None exist in Everglade today | Not applicable |

## 3. Architecture

### 3.1 Decision: the room server is the authority

Owner decision, 2026-11-03: **the server is authoritative from the start.**
No player's page ever simulates the shared world, and there is no room host,
host election or host migration.

- The room server (`coax/web/room/`, Cloud Run `oa-room`) runs one
  **Everglade room simulation** per instance (`everglade-1`, ...), in the
  same process as the relay and Orbit League, as Orbit League's rooms
  already run `orbitleague.coax` on the server.
- The simulation runs at a **fixed 30 Hz tick**. Each tick it applies every
  seat's queued intents, steps every player's movement, the 5E rules, the
  townsfolk, the watch and WANTED, spawned creatures and spell zones, fire,
  destruction and the rebuild, then writes what changed.
- It links the existing Coax, not a rewrite:
  - the SRD 5e rules unit `everglade5e-rules.coax` (generalized from one
    caster to the room's 16 seats);
  - **Everglade's headless world step**, extracted from `everglade.coax`
    and its modules (`verse/everglade/folk.coax`, `watch.coax`,
    `everglade-watch.coax`, `everglade-guard-ai.coax`, `everglade-spells.coax`,
    `everglade-abilities.coax`, `everglade-circle.coax`, the storm, frost,
    tide, fire, rubble and doors modules) into a library with no drawing:
    the city's plan, walls, bodies and ground heights, and every step that
    today takes one `you: Point` takes the room's players;
  - **one shared movement module** (`everglade-move.coax`, new): the
    character controller (`aethyr/player.coax`) plus what
    `everglade-face.c` does around it today (footing and pace, the river's
    swim, flow and breath, Ray of Frost's slow, the spell lifts, the
    bridge deck, the city's edge, `everglade_hold`), moved from the C face
    into Coax so the server and the page run the identical step.
- Clients send **intents** (move input, cast, interact, rebuild) and draw
  what the server sends: **a full snapshot on join, then deltas**, filtered
  by **interest** (section 3.6). A client predicts only its own movement,
  with the same `everglade-move.coax` step, and reconciles to the server's
  position for its last acknowledged input (section 3.9).
- Every roll, every HP change, every death, break and WANTED level is the
  server's. A patched client can only send intents, which the server checks
  like any other.

**Why not the alternatives:** a room host on a page trusts a player's machine
with everyone's world and stalls when that tab hides; lockstep needs
identical floats on every client and stalls on the slowest of 16; client
replication of casts lets each page decide what a spell did. All three are
dropped.

### 3.2 Transport

- **Primary: WebSocket** to `wss://openagents.com/api/room/relay/everglade/<room>`
  through `coaxial_socket_open` in `everglade5e-face.c`. Text frames, one
  socket a page.
- **Fallback: HTTP presence** (today's 1 Hz poll). It stays on:
  it is the server-checked roster (names, ids, the 16-player cap, automation
  filtering, blocks), feeds the homepage and the Grid, and draws others
  (with real bodies, section 5) when the socket cannot open. The poll moves
  from 1 Hz to 0.5 Hz once the socket is open.
- **Chat** stays on HTTP (`/api/playtest/chat/<room>`), where it is stored,
  rate-limited, moderated and reportable. The `[5E] ` hidden lines stop: the
  server's state carries every player's HP.

### 3.3 Relay changes (built, #12705)

In `coax/web/room/room-relay.coax`, `room.coax` and `room-host.c`:

1. Seats per room: 16 (`RELAY_SEATS`) for the `everglade` game, matching
   `verse.room_max()`; every other game keeps 4 (`RELAY_GAME_SEATS`). Seats
   and counts are written as decimal numbers (`0` to `15`), not one
   character, and the first frame is `y SEAT N T`: `T` is the server's wall
   clock in ms (Unix epoch), for clock sync. The existing games read the seat
   as a decimal field and ignore the extra field (One-Shot's `take_int`,
   `lockroom.coax`'s reader); `relay-check.coax` checks both.
2. `CONNS` 512, `RELAYS` 64, `RELAY_QUEUE` 1024. Each frame's recipients are
   fixed when it is queued. The host drains the queue after every event and
   one event queues at most 17 frames, so the queue cannot fill; a full queue
   would be counted (`rm_relay_queue_full`) and logged.
3. Per-connection output buffers (`room-host.c` `put_out`): a frame goes onto
   each recipient's buffer, sent as far as its socket takes it, the rest on
   `POLLOUT`. No send waits on one socket. A connection whose waiting bytes
   pass 256 KB (`OUT_MAX`) is closed (its seat's `l SS` goes out), so the
   slowest connection is closed instead of anyone waiting or losing frames.
4. Per-connection limits, for the `everglade` game only: 40 frames a second
   (burst 60), 1,024 bytes a frame, 16 KB a second (burst 16 KB). Over the
   limit, frames are dropped and counted (`rm_relay_dropped`, in the host's
   minute log); dropping for 10 s with no full second clear closes the socket.
   One-Shot and the lockstep games keep their unlimited rate (their GM views
   and snapshots are larger and burstier).
5. A frame `t MS` (MS: an integer) is a clock ping: the relay answers the
   sender only with `t MS T` (its MS as sent, T the server's wall clock in
   ms) and never relays it. Any other frame starting `t` relays as before.
6. Cloud Run: concurrency 1000 (from 64), memory 512 MiB (from 256), still
   max 1 instance and min 0 (`coax/web/room/deploy.sh`).

For everglade rooms the relay is the room simulation's front door, and three
things change from a plain relay room:

7. **Nothing is relayed.** A seat's frames go to the room's simulation as
   intents; no `m SS` frame is ever sent in an everglade room. The server
   writes every frame a client hears (`y`, `j`, `l`, `t` and the state
   frames in 3.6).
8. **Tickets.** Joining needs `?t=TICKET` (section 4.4). The server checks
   the ticket's signature, stamps the seat with its player id and name, and
   `j SS ID NAME` tells everyone who sat down. A socket without a valid
   ticket gets no seat (the socket is closed). Frames never claim an
   identity. Built: section 4.4.
9. **A late joiner** gets the full snapshot as its first state frame; there
   is no host to ask.

### 3.4 Rooms and sharding

- A room is an **instance** of the realm: `everglade-1`, `everglade-2`, and
  so on, 16 players each. The relay keys it as game `everglade`, code
  `<realm-slug>-<n>`; a fork's rooms are its own slug's.
- Joining: the page asks `GET /api/playtest/rooms/everglade` (built,
  #12705). The room server answers it from its own seats (the load balancer
  routes `/api/playtest/rooms/` to `oa-room`, beside `/api/room/`), so the
  answer is the same count the 16-seat cap enforces, not HTTP presence's
  6-second view:

  ```
  GET /api/playtest/rooms/SLUG
  {"realm":"SLUG","game":"everglade","seats":16,"join":"SLUG-2",
   "rooms":[{"code":"SLUG-1","players":16},{"code":"SLUG-3","players":4}]}
  ```

  `rooms` lists the live everglade rooms named `SLUG-N` (N from 1, no
  leading zero), lowest first; `join` is the lowest instance with a free
  seat, which may be one nobody is in yet. The page joins `join`, or the
  instance a link names (section 8). The relay's 16-seat cap is the hard
  limit: a full instance answers 404 to the socket and the page asks again.
- HTTP presence uses the instance's name as its room, so the roster and the
  socket agree.
- Test rooms: an automation run uses code `pair-<hex>`, so it never meets
  people (the same rule as migration 0074).

### 3.5 Who owns what

| Kind of state | Authority |
| --- | --- |
| Your position, yaw, velocity, gait, clip | The server, from your inputs; your page predicts it (3.9) |
| Your look, class, name shown | The ticket (section 4.4) and your hello |
| Casts: whether they happen, their seed, their aim | The server (an accepted `C` becomes an `e cast` event) |
| Casts' visuals (flights, blasts, marks) | Every page plays the server's `e cast` from its seed |
| Creatures and zones spells spawn | The server |
| Townsfolk, the watch, their HP, deaths, chases, WANTED | The server |
| Every roll, HP change, condition and concentration | The server (`everglade5e-rules.coax`) |
| Destruction, fire, the rebuild | The server; debris physics is each page's, from the break event |
| Weather and time of day | The server's seed and clock; every page computes it from `W` |
| Admin edits | The publish API (`/api/playtest/world/<id>`); the server reloads the world doc and sends `e edit VERSION` |

### 3.6 Frames

Text frames, space-separated fields, integers where possible (centimetres,
centimetres a second, centiradians, milliseconds). One socket a page:
`wss://openagents.com/api/room/relay/everglade/<code>?t=TICKET`. The
`realm.v1` protocol messages (`ActorAppear`, `ActorUpdate`, `SpatialDelta`,
`Snapshot`, `Command`) are the target; these text frames map onto them field
for field and are replaced when Coax gains `#import_proto` (protocol
section 13).

From the relay layer (built, 3.3): `y SS N T`, `j SS ID NAME`, `l SS`,
`t MS T`.

From a player (intents):

```
H 1 LOOK CLASS FLAGS NAME   hello, once after y: look and class (section 5), flags (1 touch), the
                            name shown until tickets carry it
I SEQ MX MZ YAW B           input, one per client frame step at 30 Hz while any input is held, else
                            5 Hz: MX MZ the move in world axes (-100 to 100, already turned by the
                            camera), YAW your facing (centirad), B buttons (1 run, 2 jump, 4 dive,
                            8 swim up, 16 sit/stand). SEQ counts up by one an input.
C SEQ SLOT AX AY AZ TGT     cast bar slot SLOT at the aim (cm); TGT -1 none, 0 to 999 a townsperson or
                            guard (watch actor index), 1000 + SS a seat, 2000 + EID a creature
E SEQ KIND ID               interact: KIND 1 talk to townsperson ID, 2 a door, 3 a boat, 4 a quest board
R SEQ                       ask for the rebuild
t MS                        a clock ping
```

From the server (state), to each seat:

```
W TICK HZ SEED DAY WEATHER  the world header, right after y: the tick now, the tick rate (30), the
                            room's seed, the time of day (ms into the cycle) and the weather schedule's
                            zone; with the y frame's T every page computes the clock, the weather and
                            the folk clock the server uses
D TICK ACK F E;E;...        state: TICK the server tick, ACK the last I SEQ of yours applied (your
                            reconciliation point), F 1 a full snapshot (on join, and after a resync)
                            else 0, then entries separated by ';'. A state over 3,800 bytes goes out as
                            D frames with F 2 (more follows) and the last with F 0 or 1.
r SEQ REASON                an intent of yours refused (range, sight, dead, cooldown, unknown slot),
                            to you only
```

Entries in `D`:

```
p SS X Y Z YAW VX VY VZ G CLIP HP MAX TEMP COND FORM     a player (always, every seat in the room);
                            G 1 on ground, 2 swimming, 4 in a boat, 8 dead; CLIP the animation clip;
                            COND a bitmask of the 15 conditions; FORM 0 you, 1-4 a Wild Shape form
f I X Z YAW CLIP MODE HP    a townsperson or guard (watch actor I), MODE its watch mode (walk, talk,
                            flee, chase, fight, down, frog)
c EID KIND X Z YAW HP F OWNER    a creature or a zone a spell made (EID the server's), OWNER its seat
x K ID                      gone from your view: K p, f or c (left, out of interest, dead and removed)
e cast SS SLOT AX AY AZ SEED     a cast the server accepted; every page plays its visual from SEED
e hit WHO BY AMT KIND       damage (WHO and BY as TGT codes), KIND the damage type
e heal WHO AMT              healing
e die WHO                   a death
e break PIECE IX IY IZ      a building piece breaks with that impulse (debris is each page's)
e burn PIECE ON             fire on a piece starts (ON 1) or burns out (0)
e rebuild                   the town stands again
e edit VERSION              the world doc changed: refetch it
w SS LEVEL                  a seat's WANTED level
```

**Interest.** Players are always in every seat's state (16 at most).
Townsfolk, guards, creatures and zones are in a seat's state while within
120 m of it; one that leaves that radius gets `x`. Within it, an entity is
written only when its quantized fields changed since what that seat last
heard (a snapshot resends all). Events reach every seat within 120 m of
where they happen, except `w`, `e edit` and `e rebuild`, which reach all.

### 3.7 Rates and the bandwidth budget

| Frame | Rate | Size |
| --- | --- | --- |
| `I` while input is held | 30 Hz | about 30 bytes |
| `I` idle | 5 Hz | about 30 bytes |
| `C`, `E`, `R` | on use | under 60 bytes |
| `D` | 15 Hz (every other tick) | 16 players about 900 bytes plus changed folk and creatures, 1 to 3 KB |
| `D` full | on join and resync | 6 to 20 KB in chunks |

Per player, with WebSocket framing: up about 1.2 KB/s, inside the relay's
everglade rate limit (40 frames and 16 KB a second); down about 20 to
45 KB/s in a full, busy room. The server's per-socket output buffer
(256 KB) holds about 6 s of that.

### 3.8 Drawing remote players: interpolation

- Keep the last 4 `p` entries per seat in a ring, stamped with the server
  tick.
- Draw each remote player at `tick_now - D`, with `D` = 100 ms + 1.5 state
  intervals (200 ms at 15 Hz), interpolated between the two entries around
  that time. Yaw goes the short way round.
- If no newer entry exists, extrapolate with the last velocity for at most
  150 ms, then hold.
- The clip comes from the entry (`CLIP`), with the gait from the
  interpolated speed (`skin_motion`), so a walk never skates.
- Townsfolk, guards and creatures are interpolated the same way.

### 3.9 Your own player: prediction and reconciliation

- **Movement:** your page runs `everglade-move.coax` on your input at once
  and keeps every `I` it sent that the server has not yet acknowledged. On
  each `D`, it takes the server's `p` for your seat as the truth for input
  `ACK`, re-runs the unacknowledged inputs on top, and eases any difference
  under 0.5 m over 100 ms (a bigger one snaps). In normal play the two agree
  exactly, because both run the same Coax step on the same inputs.
- **Casts:** your cast bar and animation start at once; the spell's flight,
  hits and numbers come from the server's `e cast` and `e hit` (one round
  trip, about 100 ms). A refused cast (`r`) stops the bar and says why, as
  `E5_CANT` does today.
- **Your HP and conditions** are the server's `p` entry; nothing is
  predicted.

### 3.10 Clock sync

Every page computes `relay_ms = Date.now() + offset`, with the offset from
the `y` frame's `T` refined by `t` pings (the lowest-round-trip sample of the
last 8). The folk clock (`folk_clock`), the guards' beats and the weather
(`weather_facts`) read `relay_ms` instead of the page's `now`, so every page
puts the townsfolk and the rain in the same place. Pose and authority frames
are stamped with it.

## 4. Identity

### 4.1 Guests (no login)

- A guest already gets a player key from `POST /api/playtest/player`, kept as
  `localStorage verse.player` (`host.js` action 5; `everglade-face.c`
  `room_out`).
- **One name everywhere** (built, #12714). `verse.player_name` is the name
  the ticket, the nameplates, chat, presence and the inbox all show
  (`coax/web/api/migrations/0116_one_player_name.sql`). It is the account's
  name when that passes the filter (`verse.name_ok`). Otherwise it is the
  guest's own name, set with `POST /api/playtest/player/name {name}` (the
  same filter; `""` goes back to the generated name). Otherwise it is
  `verse.guest_name`: two words from fixed, reviewed lists, for example
  `Willow Salmon`, the same for the same player id every time. The server is
  the one generator, and the page asks for no name.
- The look (section 5.2) comes back with the ticket (#12715).
- Clearing site data makes a new guest. That is accepted.

### 4.2 Logged-in players

- `/login` (words account) and the NIP-98 sign-in
  (`POST /api/playtest/signin`, `coax/web/api/player.c`) already move a key,
  its worlds and saves to an account. The account's name replaces the guest
  name everywhere at the next presence answer.
- The local character (look, class) moves with the key and is also saved to
  the account's progress (`/api/playtest/progress/everglade/character`), so a
  second device gets it.
- A logged-in player's nameplate is gold; a guest's is white.

### 4.3 Display names and moderation basics

- **Name filter:** 3 to 20 characters, letters, digits, spaces, `-` and `'`;
  no URLs; checked against a block list in SQL (new
  `verse.name_ok(text)`), on both the guest name and the account name. A
  refused name falls back to the generated one.
- **Mute:** local, per player, kept in local storage. A muted player's lines
  and nameplate text are hidden; their body still draws.
- **Block:** the existing `POST /api/playtest/block`
  (`migrations/0053_verse_social.sql`). A blocked player's lines and
  messages never reach you. New: their body draws as a plain hooded shape
  without a name, and their casts' visuals are not drawn for you.
- **Report:** a REPORT button on a chat line and on a nameplate, into the
  existing report queue (`/admin/reports`, as Planetary Villa's).
- **Rate limits:** chat as today (5 lines in 10 s); relay frames as in
  section 3.3; joins, 10 a minute per player key.
- **Admin kick:** an admin's `POST /api/playtest/room/<room>/kick {player}`
  (new) revokes that player's tickets for 24 hours, and the relay drops their
  socket.

### 4.4 Anti-impersonation

- **Room tickets** (built, #12712). `POST /api/playtest/room-ticket/<room>`
  needs the player key (`X-Verse-Player`). It returns
  `{"ticket","id","account","name","expires"}` (plus `hp`, `wanted`, `look`,
  `class` and `blocked` when there are any), and the ticket is
  `v2.PID.ACCT.NAME64.ROOM.EXPIRES.HP.MAC`:
  - NAME64 is `verse.player_name` (section 4.1) in unpadded base64url, `-` if
    there is none.
  - HP is the hit points the player left a room with in the last 10 minutes,
    `-` if there are none (#12715).
  - EXPIRES is 10 minutes ahead, in wall-clock milliseconds.
  - MAC is the hex HMAC-SHA256 of the text before it.

  It is signed with the Secret Manager secret `oa-room-ticket-key`, which
  `oa-api` and `oa-room` both read (`coax/web/api/player.c room_ticket`,
  `coax/web/room/room-ticket.coax`).
- **The join.** The page asks for a new ticket before each socket open
  (`everglade-net.coax`, pipe op 4, `net_ticket`) and joins with `?t=`. The
  room refuses a forged, expired or other room's ticket, so that socket gets
  no seat. The seat then carries the ticket's id, which is the id in its `n`
  entry. It also carries the ticket's name and account bit, and the hello
  cannot change them.
- **Rolling out.** `OA_ROOM_TICKET_REQUIRED=0` lets a join without a ticket
  sit with id `-` and no account bit until every page asks for tickets.
  `room/deploy.sh` defaults to `1`.

## 5. Characters

### 5.1 Bodies

- Remote players are drawn as **the same skinned Ranger you play**
  (`player/male-ranger` in the realm's pack, 75 joints, 8 clips:
  `verse/everglade/ranger.coax:11-14`), not shapes.
- How: the Ranger draw (`RANGER_DRAW`, `everglade.coax:254`) gets instances
  for the nearest remote players, and store 2 (the palette) gets one block of
  joint matrices per instance (`RANGER_JOINTS * 12` floats each). Each
  remote player has its own `Skin` playback, fed by `skin_advance` with the
  replicated clip and the interpolated speed.
- Counts: every other player in view within 80 m has a body (15 in a full
  room). The nearest 8 on a computer, 5 on a phone (as `PEOPLE_SHOWN` and
  `PEOPLE_SHOWN_PHONE`), within 25 m, have the whole mesh. The rest have the
  pack's far mesh, `player/male-ranger.lod`: the same vertices and a fifth of
  the triangles (`pack_realm.py` `far_mesh`). Farther than 80 m, a player is
  a nameplate only.
- A beast's shape (Wild Shape, Polymorph, Shapechange: `CI_FORM_*`,
  `everglade-circle.coax:51`) is in the pose flags, and everyone draws that
  form instead of the Ranger.
- Dead players lie down (the flag). The swing (clip 7) plays as a player
  casts: yours when you cast, and everyone else's when their spell's effect
  arrives from the server.
- A player whose page the server hasn't heard for 3 s (`WG_AWAY`) fades to a
  third, nameplate too, until it is heard again.
- Class gear: the druid carries a staff across their back.
- Later, the townsfolk rig (`verse/everglade/people-rig.coax`: male and
  female, 65 joints, 12 clips including spell, slash, die and knock) as a
  second body choice, so players can look like anyone in town.

### 5.2 Look and class

- **Look**: a tint on the Ranger's cloth layers and a skin tone,
  from the same tables the townsfolk use (`SKIN_TONE`, dyes), sent as
  `LOOK` in the hello (`ranger.SKIN.DYE.TRIM`). A guest's is random at first
  visit; a CHARACTER button opens a small picker (skin, cloak colour, trim)
  before you enter, with ENTER and a default.
- **Class**: the 5E engine's caster is fixed today (57 HP, AC 15,
  the realm's druid bar). The picker adds class when the 5E engine and the
  hotbar layer (`fivebar.coax`) offer more than one Everglade loadout;
  until then everyone is the same caster and the class field is `druid`.

### 5.3 Nameplates

- Over the head (`room_labels`, `everglade-room.coax`, now at the skinned
  head's height), faded by distance as today (`NAME_NEAR`, `NAME_FADE`).
- Gold for logged-in, white for guests. 
- Under the name, when hurt or under a condition: the HP bar and conditions
  that `everglade5e.coax` already draws for others (from the server's `p` entries).
- The speaker's last chat line shows over them for 6 s.

## 6. Combat, spells and the shared world

### 6.1 Who rolls and how results broadcast

The room server runs the 5E engine (`everglade5e-rules.coax`) for every cast
and blow. A player sends `C` (slot, aim, target); the server checks the slot,
range and line of sight, rolls with the room's seeded RNG, applies damage,
conditions and concentration to townsfolk, creatures and players, and writes
the outcome as `e cast`, `e hit`, `e heal` and `e die` events and the new HP in
`p`, `f` and `c` entries. Every page shows the same numbers over the same
heads. The rules' "nothing is held back" choice stays: every slot can always
be cast (no spell slots or actions run out).

### 6.2 Spell visuals

- Every visual is played locally from the server's `e cast` seed and aim:
  meteors, bolts, orbs, blasts, sparks and puffs (`SpMeteor` to `SpBolt`,
  `everglade-spells.coax`), frost, tide and storm effects. Nothing per
  particle crosses the network.
- Ground marks (`everglade-marks.coax`) follow the cast.

### 6.3 Spawned creatures and spell zones

Everything a spell or ability leaves in the world is a server entity (`c`
entries, by `EID`): Conjure Animals' pack (`everglade-circle.coax`), the
Polymorph frog, Wild Shape and Shapechange forms, Wall of Stone's panels,
frost patches and ice, the storms, fire on wood, and any later summon. The
server spawns it, moves it, applies its effects, ends it with concentration
or its duration, and keeps it when the caster leaves until the effect would
end.

Summoned creatures are creatures of the room's battle with their SRD 5.2.1
stat blocks (built, #12705): Conjure Animals brings four Wolves (AC 12, 11 Hit
Points, Bite +4 for 1d6 + 2 piercing and Prone, Pack Tactics) at the point
cast. They run on the server's tick to where their caster points (casting
again sends them) and bite the person they are set on. Every player can
target one (`C` with TGT 2000 + EID) or catch it in an area. The server rolls
every hit, and a Wolf at 0 Hit Points dies (`e die 2000 + EID`) and is gone
(`x c EID`). Each is a `c` entry of kind 12 (`WK_BEAST`): F 1 while it runs,
P1 its form (1 a wolf), P2 its most Hit Points. The SRD's own Conjure Animals
is an intangible pack, an area. The room makes it beasts that can be fought,
by the owner's decision.

### 6.4 Townsfolk and the town watch

- One town per room, on the server: each townsperson's routine
  (`verse/everglade/folk.coax`) and each guard's beat run against all of the
  room's players instead of one `you`.
- **WANTED is per player.** The server's watch (`verse/everglade/watch.coax`
  rules) keeps a crime level for each seat's player. Guards who see your
  crime chase you, and everyone sees them chase you. Another player is not
  chased for your crime, and the guards do not attack bystanders.
- The guard captain and guide decisions (`everglade-guard-ai.coax`,
  `everglade-guide.coax`) run on the server only, through the Decisions API,
  so one decision serves the room.
- A townsperson talking to one player (`E 1 ID`, then the NPC chat at
  `/api/playtest/npc`) is shown to others as standing and talking (its `f`
  MODE). The words stay private to that player.
- A late joiner gets the whole town in its snapshot.

### 6.5 PvP

**Default: off.** Spells and blows pass through other players; area damage
hurts townsfolk, creatures and the town only. Helpful spells (healing,
temporary HP) do apply to other players. The realm gets a new edit row,
`pvp` ("Players can hurt each other"), off by default, for forks and
owners who want it. A duel invitation is a later phase.

### 6.6 Destructible town state and the R rebuild

- The server decides every break (`e break` with the piece id and the
  impulse) and every fire (`e burn`). Every page runs the debris physics
  itself from that event; where a piece lands is cosmetic and need not match.
- **Rebuild:** nothing grows back by itself. Offline Everglade dropped its
  old 60 s regrow (c3b1242596, after #10941: a destroyed town snapping back
  read as an unwanted reset), so a room has none either: a broken building
  stays broken until R. **R** sends
  `R`: the server rebuilds the whole town if no spell struck it in the last
  10 s, at most once every 30 s a room, and sends `e rebuild` to everyone
  (else `r SEQ breaking` or `r SEQ cooldown`). What a fire burns through and
  what falls with it don't hold R off: a burning town would never be quiet
  for 10 s, and R puts the fire out. An admin's `R` always works.
- The bridge over the river is wood and breaks and burns as the town's
  wood does: its deck is the kit's planks (4 across the river by 7 along
  the trail) and its rails the kit's fence, pieces like any other. Where a
  plank is down there is no deck: a player or a townsperson there is in
  the river (`p` G 2), and with no row of planks whole the watch goes
  straight through the water rather than by the bridge. R stands it up with
  the rest. The beams and piles under the deck stay (they hold nothing
  anyone walks on). `bridge-check.coax` checks it through the room.
- Destruction is never saved. A room that empties starts whole.

### 6.7 Weather, time of day and the flock

- The room server owns the weather and the day cycle (#12709,
  `coax/web/room/room-sky.coax`). Its day is 48 minutes long (`WIRE_DAY_MS`) and
  turns on its wall clock. Its weather is the temperate schedule on
  `WIRE_WEATHER_SEED` at its world tick. `W` carries both: `DAY` is ms into the
  day at `TICK`, and `WEATHER` is the state plus 1. A seat hears `W` when it
  sits, and every seat hears it again when the weather turns.
- The page draws the day and the rain on the server clock (section 3.10,
  `everglade-net.coax` `net_sky_*`). Where the server's state differs from the
  page's schedule, the page pins the server's state. Offline, or with a `W`
  whose `WEATHER` is 0, the page keeps its own sky.
- Your flock (the river quest's birds) is a pose flag; other pages draw a
  flock following you with their own local flocking.

### 6.8 Admin edits

- Publishing stays on `/api/playtest/world/<id>` with its owner and admin
  checks. After a publish the server reloads the world doc (the props and
  walls it simulates against) and sends `e edit VERSION`; every page in the
  room refetches and applies it.
- Placed props join the world for everyone within 2 s of the publish.

## 7. Persistence

| What | Survives a reload? | Where |
| --- | --- | --- |
| Name, look, class | Yes | `localStorage coaxial.everglade.character`; the account's progress when logged in |
| Position and room you were in | Yes | `localStorage coaxial.everglade.place` (`place.coax`, host action 11, already) |
| Spell bar arrangement | Yes | `localStorage coaxial.everglade.bar` (host action 12, already) |
| Quests, the flock, NPC memory of you | Yes | The realm's save slot (host action 4), and the account's progress |
| HP, spell slots, conditions | Yes, for 10 minutes | The room server keeps them by player id for 10 minutes after you leave, so a reload is not a free heal; after that, full |
| WANTED | Yes, for 5 minutes | The room server, by player id, so a reload does not clear it |
| Inventory | Yes | The account's progress; a guest's save slot |
| Spawned creatures and zones | No | They end when their effect ends |
| Destruction | No | The room starts whole when it empties |
| Admin edits | Yes | The published world doc, as today |

The room server keeps the 10-minute and 5-minute records in memory (lost on
a restart; acceptable).

## 8. The Grid, the portal and links

### 8.1 Links

- `/worlds/everglade` joins the lowest instance with room.
- `/worlds/everglade?room=everglade-3` joins that instance (falling back to
  the lowest with room when it is full, with a toast).
- `/worlds/everglade?with=PLAYER_ID` joins the instance that player is in now
  (from HTTP presence).
- An **INVITE** button (beside the chat) copies
  `https://openagents.com/worlds/everglade?room=<instance>`. The hash keeps
  its meaning (a place: `#commons`), so `?room=everglade-3#market` puts a
  friend in your instance at the market.

Built (#12716, `everglade-join.coax`): the room server answers `?with=PID`
from the seats' tickets (`rm_rooms`, `"with":"SLUG-N"` when found), so a
friend's instance comes from the server that seats you, not from HTTP
presence. A named instance that refuses the socket twice gives way to the
list's `join`, with a toast. INVITE and CHARACTER sit left of MAP (on a
phone, a row under it). CHARACTER picks the skin tone and the cloak and trim
dyes; it opens before the socket on a first visit (no look came back with the
ticket), and the class row says the druid is the one class for now. A guest's
name is two words from a fixed list, hashed from the player id, sent with the
ticket's ask (an account's name outranks it). Your own Ranger draws in your
look, as the others see you.

### 8.2 The Grid hand-off

- The Grid (`/worlds/the-grid`, `newland-walk.coax`) has an Everglade gate
  ahead and to the right of the spawn, facing it. Walking through its portal
  navigates to `/worlds/everglade?from=grid&with=<friend>`, the friend being
  the last other walker within 8 m of you in the last 2 minutes; with none,
  `/worlds/everglade?from=grid`. When the friend went in first, the room
  server seats you in their instance; else the lowest with room.
- `from=grid` plays a short arrival (a veil that lifts). The server's spawn
  is already the road in at the South Gate, facing in.
- Not built: a way back. Everglade has no HOME button yet.

## 9. Scale and cost

### 9.1 Expected load

- Today's realms see tens of visitors a day. Plan for a launch peak of 50
  concurrent players in Everglade and design for 256 a relay instance (16
  full rooms).

### 9.2 Relay capacity

- Measured (2026-11-03, `load.coax everglade`, 64 players at 15 frames a
  second each fanned out to 15): the relay's own time over a socket's bytes
  is p50 0.2 ms and p95 0.7 ms on Cloud Run's 1 vCPU (`/api/room/health`).
- With the room simulation, a full room is 16 players x 30 inputs a second
  in, one 30 Hz tick, and 15 state frames a second out to each of 16 seats.
  The tick's budget is 2 ms (16 rooms in one core at 30 Hz with headroom).
  The tick is measured and reported in `/api/room/health` once built.
- Memory: `IN_CAP` 16 KB a connection x 512 = 8 MB, plus output buffers.
  512 MiB is ample.
- Shards (#12717): one process's 30 Hz tick fits about three full rooms on
  Cloud Run's 1 vCPU (four rooms took 78% of it and the tick fell to 28 Hz),
  and the host is one thread, so more vCPU would not help. Everglade's
  instances run on six services of one container each, `oa-room` (shard 1)
  and `oa-room-2` to `oa-room-6`, min 0 and max 1 each. The API's directory
  (`coax/web/api/rooms.c`, migration 0115) places each instance on the lowest
  shard under its cap of two (rooms_cap), holds a seat 20 s from the ticket, and the
  ticket (v3) names the shard. The page opens `/api/room/sK/relay/everglade/CODE`,
  which the load balancer sends to shard K, and a shard seats only its own
  tickets. Shards post their rooms and players to
  `POST /api/playtest/room-directory` every 5 s (`room-directory.coax`), and
  `GET /api/playtest/rooms/REALM` is the API's, from the directory.
  Session affinity was rejected: it is a per-browser, best-effort cookie, so
  two friends could land on two containers.

### 9.3 Cost

All on project `openagentsgemini`, no third-party realtime service.

| Item | Estimate |
| --- | --- |
| `oa-room`, 1 vCPU and 512 MiB, billed while any socket is open (min 0, max 1). Worst case always on | about $65 a month |
| Shards `oa-room-2` to `oa-room-6` (#12717), the same size, min 0 and max 1 each. Shard K runs only while more than 2(K-1) instances are open (two a shard: the town sim fits two full rooms in the 33 ms tick), so at the expected 50 players shards 1 and 2 run. Worst case, all five always on | about $65 each, $325 a month more |
| The server simulation: at 30 Hz a full room's tick must fit in about 2 ms of one vCPU so 16 rooms fit one core. If measured ticks need 2 vCPU, worst case always on | up to $65 a month more |
| Egress: a full room is about 16 x 45 KB/s = 720 KB/s. At an average of one full room for 2 hours a day, about 160 GB a month | about $13 to $19 a month |
| HTTP presence load on Cloud SQL drops (1 Hz to 0.5 Hz per player) | no new cost |
| Secret Manager secret for tickets | under $1 |

**Total added: under $150 a month worst case, about $20 to $40 at expected
load** (min instances stays 0). This must fit inside the hard rule of under
$1,500 a month total GCP spend; check the current total before raising
max instances or CPU, and set a budget alert on `oa-room`. Accelerators are
not used.

## 10. Security

- **Validate every frame at the edge.** The relay enforces sizes, rates and
  identity (tickets). Clients parse frames with bounded readers (as
  `room_take` does) and ignore unknown kinds.
- **Movement is the server's.** A client sends inputs, not positions; the
  server's `everglade-move.coax` step decides where it goes, so speed and
  teleport cheats have nothing to send. Input values are clamped to their
  ranges and at most 40 frames a second are taken (the relay's limit).
- **Warps** (a hash place, a fork arrival, respawn) are the server's: a
  hello may ask for a named place, at most once every 5 s.
- **Casts** are intents: the server refuses an unavailable slot,
  a spent action, an aim out of range or out of sight, and more than 4 casts
  a second.
- **Entities:** at most 16 owned entities a player, 128 a room; a spawn past
  the cap is refused.
- **Abuse:** chat limits as today; names filtered (section 4.3); mute, block,
  report and admin kick; automation isolated in `pair-` rooms and kept out of
  analytics (migration 0074); joins limited per key and per IP at the load
  balancer (Cloud Armor rate rule on `/api/room/relay/`).
- **Privacy:** frames carry player ids and names only, never keys or emails.
  The relay logs counts, not frame contents.

## 11. Testing

### 11.1 Deterministic multi-client tests in Coax

- **Relay** (built): `coax test --library coax/web/room/room.coax --steps
  4000000000`: 16 everglade seats with two-digit seats, `y` with the clock,
  One-Shot's 4 seats, `t` pings, the rate limits' drops and close, and
  `GET /api/playtest/rooms/SLUG`. `relay-check.coax URL slow` checks that
  a socket that never reads is closed while the others get every frame.
- **The room simulation** (to build): `#test`s in
  `coax/web/room/everglade-room-sim.coax` drive two and sixteen seats'
  intents through the 30 Hz tick in one process. Checks:
  - a seat's inputs move it exactly as the page's `everglade-move.coax`
    step predicts, so reconciliation is a no-op in normal play;
  - A's cast hits a guard; both seats' `D` carry the same HP and the same
    guard chasing A. A is WANTED; B is not;
  - A's Conjure Animals pack appears in both seats' state and stays after A
    leaves until it would end;
  - A breaks a wall; B hears `e break`; `R` rebuilds it under the 10 s and
    30 s rules;
  - a late joiner's first `D` is a full snapshot with the broken wall, the
    hurt townsfolk and the pack;
  - a townsperson more than 120 m from B is never in B's state, and gets
    `x` when it leaves B's radius;
  - a forged ticket is refused before the upgrade.

### 11.2 Headless two-browser smoke

- Extend `coax/web/presence-pair.coax` with `--socket`: it hooks
  `WebSocket.prototype.send` and `onmessage` instead of `fetch`, both
  sessions share a `pair-<hex>` code, and one holds W while the other times
  the first `P` frame that moved it. Budget: 300 ms at the 95th percentile.
- A new `coax/web/everglade-pair.coax` smoke on staging, run by
  `smoke.sh` after a release:
  1. Two headless Chromes open `/worlds/everglade?room=pair-<hex>`.
  2. Each page's debug export (`everglade_remote_count`, new) reports one
     remote body drawn; a screenshot shows a Ranger, not a cone.
  3. A casts Conjure Animals; B's export reports the creatures.
  4. A breaks a crate; B's export reports it broken.
  5. Neither session appears in real players' rooms or in analytics.

### 11.3 Load tests

- `coax/web/room/load.coax` has an `everglade` mode (built, #12705):
  `coax/web/tool.sh coax/web/room/load.coax everglade URL [64] [20]` from the
  repository's root. N simulated players (default 64, up to 256) join
  `pair-load-<stamp>-<k>` rooms in 16s, one process a room (a Coax process
  opens at most 16 sockets), send `P` at
  15 Hz with random walks, a `C` every 5 s and an `S`/`D` pair every 20 s.
- It passes when every socket opens, the server-side fan-out time is under
  50 ms at the 95th percentile (each `P` carries its send time; fan-out is
  the 95th percentile of its time across the relay less the median `t` ping
  round trip), no frame is dropped below the rate limit,
  and each client hears at least 14 frames a second from every other seat.
- `deploy.sh` runs it (64 players, 20 s) through `wss://openagents.com`
  after each deploy, beside the Orbit League load test and the relay check.

## 12. Deprecating the old Everglade

1. **The swap.** In `apps/openagents-web/realms.json`, the `everglade`
   realm's `module` becomes `everglade5e`; its slug, slot (`everglade`),
   style, voice, assets and edit rights stay. The page is built from
   `everglade5e.coax` with the realm slug `everglade`.
2. **Room and world names from the realm.** `E5_ROOM` (`everglade5e.coax:28`)
   is replaced by the realm's slug from the page facts, so `/worlds/everglade`
   uses rooms `everglade-<n>`, a fork uses its own slug's rooms, and the
   published world it loads is `everglade`'s.
3. **Saved data.** The slot stays `everglade`, so kept places, bars and quest
   saves keep working. The `everglade-5e` slot's data is carried over with
   `slot_was` (`coax/web/realms.ts:69`) on the page that the old URL redirects
   to, for players who only played 5E.
4. **Edits.** `everglade`'s published world (the live one) is what loads.
   `everglade-5e`'s world was copied from it on its split
   (`migrations/0112_everglade_5e_realm.sql`); an owner who wants any edits
   made since on `everglade-5e` copies them over before the swap (see the open
   questions).
5. **Forks.** Forks whose base is `everglade` (`verse.fork_bases`) run the 5E
   module from the swap, with their own rule sets. Forks of `everglade-5e`
   keep their slug and rooms; their base row points at `everglade`.
6. **Redirects.** `/worlds/everglade-5e` answers 301 to `/worlds/everglade`,
   keeping the query and hash. The `everglade5e` realm entry becomes
   unlisted and redirected; its slug stays reserved
   (`verse.builder_reserved()`).
7. **The old module.** `everglade.coax` stays as the library 5E loads (it is
   the city). Its own page, `everglade-face.c`, and the `everglade` wasm in
   `build.sh` are removed after two weeks with no regressions reported. The
   goldens for `everglade` are retaken from the 5E page.
8. **Copy.** The realm card's description and share line in `realms.json`
   change to say the spells roll on the 5E rules and that other players are
   there with you.

## 13. Build list and status

One architecture, built whole (owner, 2026-11-03: no interim phases, no
room host, no caster replication). Status as of the last edit of this file.

| # | Item | Where | Status |
| --- | --- | --- | --- |
| 1 | Relay: 16 decimal seats, the clock in `y`, 512 connections, queue 1024, per-socket output buffers, rate limits, `t` pings (3.3) | `coax/web/room/` | **Built and deployed** (#12705), `oa-room` concurrency 1000, 512 MiB |
| 2 | `GET /api/playtest/rooms/SLUG` and instance joining (3.4) | `room-relay.coax` `rm_rooms`, load balancer rule | **Built and deployed** (#12705) |
| 3 | The 64-player load test (11.3) | `load.coax everglade` | **Built**; since the server became the world it sends inputs (I at 30 Hz, a cast every 5 s) and checks the state frames (D at 15 Hz), every seat seen, the ACKs and the input-to-ACK time |
| 4 | The shared movement step: the controller and the face's movement glue in Coax, shared by page and server | `everglade-move.coax`, `everglade-step.coax` (#12706) | **Built** (0c52335252); the server runs it for every seat in `room-world.c` |
| 5 | Everglade's headless world step for N players (town, watch, spells, creatures, zones, fire, destruction, rebuild) | `everglade.coax` linked headless in `oa-room` (`room-world.c`, `everglade-watch.coax` `everglade_room_*`) | **Built**: each simulated room has its own headless Everglade, and the seats walk and swim in it with their own conditions (Hold Person, Ray of Frost, a blast's throw, the rules' Restrained and the rest). The town is the server's (#12713): everyone's day and the watch stepped against all 16 seats (`watch_tick_room`, each hunter after its own foe), a seat's harm to the town a crime for that seat, its WANTED level (`w`, every seat's), the guards' blows through the rules, the people as `f` within 120 m. So are Conjure Animals' Wolves (they set on a person only when their caster sends them), and the town's pieces: Everglade's four spells fall on the server's town from the cast's seed, fire burns and spreads there, pages hear `e break` and `e burn` (a snapshot names what is down), and R rebuilds under 6.6's rules. A new room starts its town whole. The river is the room's too: each seat swims, dives and is carried by the current as a page is (its own water, feet and breath; out of it on the bridge and in Part Water's trench), comes to on the bank at the sixth level of Exhaustion, and its `p` says so (G 2). The bridge breaks and burns with the town (6.6): where its planks are down a seat or a townsperson is in the river. Not yet the server's: the 60 s regrow, an admin's edits to the realm, the rain's rise on the river, Water Breathing, the boats |
| 6 | `everglade5e-rules.coax` for 16 seats | `everglade5e-room-rules.coax` | **Built** (d2852ee336): one battle, each seat its own caster and turn, zones with ids, PvP off |
| 7 | The room simulation: 30 Hz tick, intents, snapshot and deltas, interest (3.1, 3.6) | `coax/web/room/room-everglade.coax` | **Built**: wire v2 (`everglade-wire.coax`), 16 rooms of 16, inputs stepped one a step with at most 4 owed, casts checked and rolled, players, names, creatures and zones within 120 m, events, snapshots; `sim-check.coax` checks two sockets sharing one world |
| 8 | Tickets: signed seat identity (4.4, 3.3 item 8) | API and `room` | Not built |
| 9 | The page as a viewer plus its predicted movement (3.8, 3.9) | `everglade-net.coax` (#12706, #12708) | In progress elsewhere |
| 10 | Remote players as skinned Rangers (section 5) | #12707 | In progress elsewhere |
| 11 | Scale across instances: room code to `oa-room` instance routing | API directory, `room-directory.coax`, deploy and load balancer | **Built** (#12717): six shards of two instances (12, 192 players), v3 tickets name the shard, `fanout-check.coax` |
| 12 | The swap to `/worlds/everglade` (section 12) | site | Not built |
| 13 | The Grid's gate, links, INVITE and CHARACTER (5.2, 8) | `everglade-join.coax`, `newland-walk.coax`, `rm_rooms ?with=` | **Built** (#12716): the page joins the instance a link names, else the list's `join` (with `?with=` the friend's instance); a refused socket asks the list again; the gate on the Grid sends you with the friend you walked with |

Acceptance, all on production:

1. Two browsers, one a fresh guest, open `/worlds/everglade`, land in the
   same instance, and each sees the other as a skinned Ranger with a name,
   moving with the right clips, within 250 ms at the 95th percentile.
2. A hits a guard; B sees the guard's HP drop by the same number, and both
   see the same guard chase A. B is not WANTED.
3. A casts Meteor Swarm and Conjure Animals. B sees the meteors fall at the
   same spot and the pack appear, move and vanish when A's concentration
   ends; the pack stays if A leaves until it would end.
4. A breaks a house; B sees it broken. R by either rebuilds it for both,
   under the 10 s and 30 s rules.
5. A late joiner C sees the broken house, the hurt townsfolk, A's pack and
   the chase within 1 s of joining.
6. A patched client cannot change any HP, roll or position beyond what its
   inputs allow; a forged identity is refused at join.
7. The room load test passes with 64 simulated players, with the server's
   tick p95 under 5 ms at 16 full rooms.
8. A phone holds 60 FPS with 5 remote bodies in view.

Load test results (item 3), 2026-11-03, 64 players in 4 rooms of 16, 20 s:
locally every socket opened, no gap, 14.9 P a second from every seat,
fan-out p95 2 ms; through `wss://openagents.com` every socket opened, no gap,
14.9 a second from every seat, end-to-end p50 about 64 ms and p95 about
190 ms (goal 3: under 250 ms), the relay's own time over a socket's bytes p50 0.2 ms and
p95 0.7 ms (`/api/room/health`), on revision `oa-room-00012-qbp`.

## 14. Open questions for the owner

1. **Instance size.** 16 players an instance, matching the room cap today.
   Larger (32) means busier streets and twice the bandwidth. Keep 16?
2. **PvP.** Off by default with a `pvp` row for forks and owners. Agreed?
3. **Edits made on `everglade-5e` since its split.** Copy them onto
   `everglade` before the swap, or drop them?
4. **The old Everglade page.** Remove it two weeks after the swap, or keep it
   unlisted at `/worlds/everglade-classic`?
5. Answered (2026-11-03): no room host; the server is authoritative from
   the start (section 3.1).
6. **Guest names.** Generated two-word names (`Mossy Heron`) instead of
   `Guest ABCD`, and guests may pick their own through the filter. Agreed?
7. **Everyone looks like the Ranger**, told apart by tints. Is the
   townsfolk rig (male and female bodies) needed at launch?
8. **R rebuild.** Anyone may rebuild the town under the 10 s and 30 s rules,
   or admins only?

## 15. Where the Aethyr book and the protocol are out of date

The book is not rewritten here. These are the differences to fix when its
chapters are next revised.

- **Chapter 49, rooms and presence.** "What exists today" lists the 1 Hz HTTP
  poll, Fairy Forest's 0.25 s posts and the relay. It misses the Grid's 0.2 s
  posts (`newland-walk.coax`), Everglade's hooded figures, and Everglade 5E's
  vitals sent as hidden chat lines. Its design (WebSocket with HTTP fallback,
  a fixed draw delay, the nearest `k` drawn) matches this spec. It does not
  cover identity on the socket (tickets), clock sync, or the relay's 4-seat,
  one-digit and 64-socket limits.
- **Chapter 50, shared state over the network.** It offers two models,
  lockstep and server authority. It does not cover a client as the authority
  with migration (One-Shot's pattern), nor interest limits for non-player
  characters.
- **Chapter 58, Everglade.** It says others are seen; it does not say that
  the townsfolk, the watch and destruction are each page's own.
- **The Verse protocol, section 6.3.** It lists Verse multiplayer as a 60 Hz
  authoritative tick. This spec uses a 30 Hz server tick and 15 Hz
  state frames with interest; the tick profile in `sim.v1` should say so. Section 10.8's `realm.v1` remains the
  target wire; the text frames in section 3.6 are its bootstrap.
- **`docs/verse/everglade.md`.** "Hostility is your page's alone" (line 789)
  and "nothing broken is saved or shared" (line 661) become untrue when the
  room simulation ships; update them then.
