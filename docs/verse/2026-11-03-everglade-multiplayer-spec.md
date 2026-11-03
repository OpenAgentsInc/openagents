# Everglade multiplayer specification

Date: 2026-11-03. Status: specification. Phase 1 is meant to ship today.

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
13. Phased build order
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
  `openagentsgemini`, revision `oa-room-00010-fft`, 1 vCPU, 256 MiB,
  min 0 and max 1 instance, concurrency 64, timeout 3600 s, session affinity
  (`coax/web/room/deploy.sh`). The load balancer routes `/api/room/` on
  openagents.com to it. It answers on `wss://openagents.com/api/room/...`
  today.
- Limits that block Everglade:
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

### 3.1 Decision: one authority per room, on the relay, moving to the server

**Recommendation.** Use the WebSocket relay the repo already runs, extended
into an Everglade room. Each room has exactly one **world authority**:

1. **Phases 1 and 2: a room host.** One player's page, chosen by the relay,
   simulates the shared world (townsfolk, watch, spawned creatures, zones,
   damage) and sends snapshots and deltas. When it leaves or goes to a hidden
   tab, the relay names the next host, which continues from the last state it
   holds. This is One-Shot's pattern (the GM's page as authority), made
   automatic.
2. **Phase 3: the 5E rules on the server.** The SRD engine is already its
   own C unit (`everglade5e-rules.coax`). The room server links it, as it
   links `orbitleague.coax` today, and resolves every attack, save, damage
   and HP change.
3. **Phase 4: the whole world on the server.** The room server runs
   Everglade's headless step (townsfolk, watch, zones, destruction). Clients
   become pure viewers of the world plus owners of their own movement.

The frames do not change between these phases. Only who sends the authority
frames changes, so clients never notice the move.

**Why this order:**

- It ships today. The relay, the host's socket ABI and the Cloud Run service
  exist. A room host needs no new server code beyond the relay changes in
  section 3.3.
- Everglade's step is not separable yet. It lives in a 4,623-line page module
  (`everglade.coax`) mixed with drawing, plus about 30 spell and town
  modules. Pulling a headless step out is real work (phase 4). The 5E rules
  are already separate, so they can move first (phase 3).
- The world is mostly deterministic already. Townsfolk routines, the guards'
  beats and the weather are pure functions of a seed and the clock. So a host
  only sends what departs from that baseline (who is hurt, chasing, dead,
  frogged, broken), which keeps frames small and makes host migration cheap.
- **Not lockstep.** Lockstep needs every client to run identical floating
  point on identical builds (`sim.v1` says cross-machine float determinism is
  not assumed), stalls on the slowest of up to 16 players, and handles
  mid-session joins poorly. It suits Belt 2200's two seats, not a town.
- **Not a server sim on day one.** It would block multiplayer for weeks on
  the extraction work.

**What the room host costs us, and the mitigations:**

- A host can cheat on world state (kill townsfolk, clear its own WANTED).
  Mitigations: nothing has stakes yet (no economy, no XP from host results,
  PvP off); a host cannot move another player or change another player's HP
  after phase 3; phases 3 and 4 remove host trust.
- A host's frame rate is everyone's world rate. Mitigations: the relay
  prefers a host that said it can host (a computer, a visible tab, at least
  50 FPS), and a host that goes hidden or drops below 30 FPS for 5 s yields.
- A browser throttles a hidden tab. The host yields on message 601 (hidden),
  before throttling starts.

### 3.2 Transport

- **Primary: WebSocket** to `wss://openagents.com/api/room/relay/everglade/<room>`
  through `coaxial_socket_open` in `everglade5e-face.c`. Text frames, one
  socket a page.
- **Fallback: HTTP presence** (today's 1 Hz poll). It stays on in every phase:
  it is the server-checked roster (names, ids, the 16-player cap, automation
  filtering, blocks), feeds the homepage and the Grid, and draws others
  (with real bodies, section 5) when the socket cannot open. The poll moves
  from 1 Hz to 0.5 Hz once the socket is open.
- **Chat** stays on HTTP (`/api/playtest/chat/<room>`), where it is stored,
  rate-limited, moderated and reportable. The `[5E] ` hidden lines stop in
  phase 1.

### 3.3 Relay changes (phase 1)

In `coax/web/room/room-relay.coax` and `room-host.c`:

1. Seats per room: 16 (`RELAY_SEATS`) for the `everglade` game, matching
   `verse.room_max()`; other games keep 4. Seats and counts are written as
   decimal numbers (`0` to `15`), not one character, and `y SEAT N T` carries
   the server's wall clock `T` in ms for clock sync. The existing games
   already read the seat as a decimal field (One-Shot's `take_int`,
   `lockroom.coax`'s reader), and `relay-check.coax` checks both.
2. `CONNS` 512, `RELAYS` 64, `RELAY_QUEUE` 1024. A full queue closes the
   slowest connection instead of dropping frames for everyone.
3. Per-connection output buffers: no `send_all` that waits on one socket.
   A connection whose buffer passes 256 KB is closed.
4. Per-connection limits: 40 frames a second (burst 60), 1,024 bytes a frame
   for the `everglade` game, 16 KB a second. Over the limit, frames are
   dropped and counted; over it for 10 s, the socket is closed.
5. `t MS` answers a client's `t MS` ping with the server's clock, for
   round-trip and offset.
6. Cloud Run: concurrency 1000 (from 64), memory 512 MiB (from 256), still
   max 1 instance so one room lives in one process.

In phase 2:

7. **Addressed frames.** A frame that starts `@SEAT ` goes only to that seat.
   The host uses it for a late joiner's snapshot.
8. **Authority.** The relay keeps one authority seat per room and announces
   it: `a SS`. It picks the earliest-joined seat whose hello said `host=1`,
   else the earliest seat. It moves authority on leave, on `yield`, or on a
   ping silence of 3 s, and announces the new one.
9. **Tickets.** Joining needs `?t=TICKET` (section 4.4). The relay stamps the
   seat with the ticket's player id and name, and `j SS ID NAME` tells
   everyone who sat down. Frames can no longer claim an identity.

### 3.4 Rooms and sharding

- A room is an **instance** of the realm: `everglade-1`, `everglade-2`, and
  so on, 16 players each. The relay keys it as game `everglade`, code
  `<realm-slug>-<n>`; a fork's rooms are its own slug's.
- Joining: the page asks `GET /api/playtest/rooms/everglade` (new, phase 1)
  for each instance's count from HTTP presence and joins the lowest instance
  with room, or the instance a link names (section 8). The relay's 16-seat
  cap is the hard limit; a full instance answers 404 and the page tries the
  next.
- HTTP presence uses the instance's name as its room, so the roster and the
  socket agree.
- Test rooms: an automation run uses code `pair-<hex>`, so it never meets
  people (the same rule as migration 0074).

### 3.5 Who owns what

| Kind of state | Authority | Phase |
| --- | --- | --- |
| Your position, yaw, velocity, gait, clip | You (client-authoritative, checked, section 10) | 1 |
| Your look, class, name shown | The roster (HTTP presence, then the ticket) | 1, 2 |
| Your casts' visuals (flights, blasts, marks) | You send the intent; every page plays it from the same seed | 1 |
| Creatures and zones you spawn | You (owner-simulated) in phase 1; the room authority from phase 2 | 1, 2 |
| Townsfolk, the watch, their HP, deaths, chases, WANTED | Room authority | 2 |
| Combat resolution and every HP change | Room authority in phase 2; the room server from phase 3 | 2, 3 |
| Destruction, debris, fire, rebuild | Room authority | 2 |
| Weather and time of day | Nobody: computed from the seed and the relay's clock | 1 |
| Admin edits | The publish API (`/api/playtest/world/<id>`), as today | 2 |

### 3.6 Frames

Text frames, space-separated fields, integers where possible (centimetres,
centiradians, milliseconds). The relay prefixes `m SS ` on the way out. The
`realm.v1` protocol messages (`ActorAppear`, `ActorUpdate`, `SpatialDelta`,
`Snapshot`, `Command`) are the target; these text frames map onto them
field for field and are replaced when Coax gains `#import_proto` (protocol
section 13).

From a player (phase 1):

```
H 1 ID LOOK CLASS FLAGS NAME     hello: on join, and again when anyone joins (j)
P SEQ T X Y Z YAW VX VZ F A      pose: T ms (relay clock), X Y Z VX VZ cm and cm/s, YAW centirad,
                                  F flags (airborne, swim, boat, dead, form 0-4, flock), A clip
C SEQ SLOT AX AY AZ SEED TGT     cast: a bar slot, the aim (cm), the seed every page plays it with,
                                  the target (-1 none; a townsperson's index or a seat)
S EID KIND X Z YAW HP F          a creature or zone you own: spawn or update (EID: seat*256 + n)
D EID                            it is gone
V HP MAX TEMP COND               your vitals, on change (self-asserted until phase 2)
t MS                             a clock ping
```

From the room authority (phase 2):

```
@SS A s K N CHUNK                a snapshot for a late joiner, chunk K of N (base64, under 1 KB each)
A d TICK ...                     a delta: townsfolk and creatures that changed, as
                                  "f I X Z YAW HP CLIP" / "c EID ..." / "x EID" (gone) entries
A e TICK KIND ...                an event: a hit (who, by, amount, kind), a death, a break
                                  (piece id, impulse), a rebuild, a crime (player, level)
A w PLAYER LEVEL                 a player's WANTED level
A v SEAT HP MAX TEMP COND        a player's vitals, decided by the authority
yield                            the host steps down (hidden tab, low frame rate)
```

From a player to the authority (phase 2): `I SEQ SLOT AX AY AZ TGT`, a cast
intent, and `R`, a rebuild request. The authority answers a refused intent
with `@SS A n SEQ REASON`.

### 3.7 Rates and the bandwidth budget

| Frame | Rate | Size |
| --- | --- | --- |
| `P` while moving | 15 Hz | about 60 bytes |
| `P` while still | 2 Hz | about 60 bytes |
| `C`, `S`, `D`, `V` | on change | under 80 bytes |
| `A d` | 10 Hz, only what changed within 120 m of anyone | 0.5 to 2 KB |
| `A s` | once a joiner, plus every 30 s as a checkpoint | 6 to 20 KB in chunks |

Per player, with WebSocket framing:

- Up: about 1 KB/s.
- Down, in a full room of 16: 15 others at about 1 KB/s each, plus up to
  3 KB/s of authority deltas, so at most about **20 KB/s (160 kbit/s)**.
  That is fine on a phone. The host sends about 3 KB/s more.

### 3.8 Drawing remote players: interpolation

Replace `room_ease` for socket players:

- Keep the last 4 `P` samples per seat in a ring, stamped with the relay
  clock.
- Draw each remote player at `now - D`, with `D` = 100 ms + 1.5 send
  intervals (200 ms at 15 Hz), interpolated between the two samples around
  that time. Yaw goes the short way round.
- If no newer sample exists, extrapolate with the last velocity for at most
  150 ms, then hold. Never extrapolate through a wall: clamp to
  `walk_at`'s ground.
- A seat with no `P` for 3 s fades out. One with none for 10 s is removed.
- The clip comes from the sample (`A`), with the gait from the interpolated
  speed (`skin_motion`), so a walk never skates.
- HTTP-only players (no socket) keep easing, with `D` = 1.5 s.

### 3.9 Your own player: prediction and reconciliation

- **Movement:** client-authoritative. You move at once with no round trip,
  and nothing corrects you in normal play. A failed speed or teleport check
  (section 10) makes the authority send `@SS A m X Z`, and your page snaps
  there.
- **Casts:** you predict. The cast bar, your animation and the flight start
  at once. In phase 1 your page also decides what your spell does to
  townsfolk on your screen only. From phase 2 the authority decides hits,
  damage and HP, and your page shows the numbers when `A e` arrives (one
  round trip, about 100 ms). A refused intent (`A n`) stops the flight and
  says why under the bar, as `E5_CANT` does today.
- **Your HP:** from phase 2 the authority's `A v` is the truth. Your page
  shows the predicted value and snaps to the authority's when they differ.

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
  `room_out`). Its server name is `Guest` plus four letters.
- New: on first visit the page makes a **name** (two words from a fixed,
  reviewed list, for example `Mossy Heron`) and a **look** (section 5.2), and
  keeps both in `localStorage coaxial.everglade.character`. The name is sent
  to `POST /api/playtest/player/name` (new) so the server-side name, which
  everyone sees, is the same.
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

- **Phase 1** (no tickets yet): a socket's hello names a player id. A page
  draws a seat with a name only if that id is in the HTTP presence roster
  (server-checked, keyed by the unforgeable player key) **and** the seat's
  pose is within 5 m of the roster's position for it. A seat that fails is
  drawn as a nameless figure. Two seats claiming one id: only the one that
  matches is named.
- **Phase 2: room tickets.** `POST /api/playtest/room-ticket/<room>` (player
  key required) returns `base64(player_id | name | room | expires | HMAC)`,
  valid for 10 minutes, signed with a secret in Secret Manager
  (`oa-room-ticket-key`) that both `oa-api` and `oa-room` read. The relay
  checks it on join and stamps the seat. Nothing a client sends can change
  its id or name after that.

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
- Counts: 8 remote bodies on a computer, 5 on a phone (as `PEOPLE_SHOWN` and
  `PEOPLE_SHOWN_PHONE`). Others beyond that, or farther than 80 m, show a
  nameplate only.
- A beast's shape (Wild Shape, Polymorph, Shapechange: `CI_FORM_*`,
  `everglade-circle.coax:51`) is in the pose flags, and everyone draws that
  form instead of the Ranger.
- Dead players lie down (the flag), and the sledgehammer's swing is clip 7.
- Phase 4 adds the townsfolk rig (`verse/everglade/people-rig.coax`: male and
  female, 65 joints, 12 clips including spell, slash, die and knock) as a
  second body choice, so players can look like anyone in town.

### 5.2 Look and class

- **Look** (phase 1): a tint on the Ranger's cloth layers and a skin tone,
  from the same tables the townsfolk use (`SKIN_TONE`, dyes), sent as
  `LOOK` in the hello (`ranger.SKIN.DYE.TRIM`). A guest's is random at first
  visit; a CHARACTER button opens a small picker (skin, cloak colour, trim)
  before you enter, with ENTER and a default.
- **Class** (phase 2): the 5E engine's caster is fixed today (57 HP, AC 15,
  the realm's druid bar). The picker adds class when the 5E engine and the
  hotbar layer (`fivebar.coax`) offer more than one Everglade loadout;
  until then everyone is the same caster and the class field is `druid`.

### 5.3 Nameplates

- Over the head (`room_labels`, `everglade-room.coax`, now at the skinned
  head's height), faded by distance as today (`NAME_NEAR`, `NAME_FADE`).
- Gold for logged-in, white for guests. A small `HOST` marker shows only in
  the debug overlay, never to players.
- Under the name, when hurt or under a condition: the HP bar and conditions
  that `everglade5e.coax` already draws for others (from `A v` from phase 2).
- The speaker's last chat line shows over them for 6 s.

## 6. Combat, spells and the shared world

### 6.1 Who rolls and how results broadcast

- **Phase 1:** each page rolls its own player's casts on its own screen, as
  today. Others see the cast (its flight, its blast, its marks) from the `C`
  frame, played with the same seed, so it looks the same. What it does to
  townsfolk differs per page until phase 2.
- **Phase 2:** the room authority runs the 5E engine for every cast. A
  player sends `I` (slot, aim, target); the authority checks the slot, the
  action economy, the range and the line of sight, rolls (its seeded RNG),
  applies damage, conditions and concentration, and broadcasts the outcome
  as `A e` hit events and `A v`/`A d` state. Every page shows the same
  numbers over the same heads.
- **Phase 3:** the same, run by the room server (`everglade5e-rules.coax`
  linked into `room`), so no client decides any roll.

### 6.2 Spell visuals

- Every visual is played locally from the cast's seed and aim: meteors,
  bolts, orbs, blasts, sparks and puffs (`SpMeteor` to `SpBolt`,
  `everglade-spells.coax:140-151`), frost, tide and storm effects. Nothing per
  particle crosses the network.
- A visual starts when `C` (or, from phase 2, the authority's accepted
  intent) arrives, offset by the sender's latency so its impact lands at the
  same relay time on every page.
- Ground marks (`everglade-marks.coax`) follow the cast.

### 6.3 Spawned creatures and spell zones

Everything a spell or ability leaves in the world is an **entity with an id**
(`EID`) that every page draws:

- Conjure Animals' pack (`ci_ca_at`, `everglade-circle.coax:264`), the
  Polymorph frog, Wild Shape and Shapechange forms, and any later summon.
- Lingering zones: Wall of Stone's panels (`everglade-rubble.coax`), frost
  patches and ice (`FrPatch`, `FrIce`), Sleet Storm and the other storms
  (`everglade-storm.coax`), fire on wood (`everglade-fire.coax`), Black
  Tentacles, Moonbeam and Spike Growth where the bar has them.

Authority by phase:

- **Phase 1:** the caster's page simulates its own creatures and zones and
  sends `S` (spawn and updates at 5 Hz for moving creatures, once for still
  zones) and `D`. Every page draws them. A caster who leaves takes theirs
  with them (`l` despawns the seat's entities).
- **Phase 2:** the room authority simulates every creature and zone. Casting
  one is an intent; the authority spawns it, moves it, applies its effects to
  townsfolk and players, ends it with concentration, and keeps it when the
  caster leaves until the effect would end.

### 6.4 Townsfolk and the town watch

- One simulation per room (phase 2), on the authority. Routines are still a
  function of the seed and the relay clock, so a townsperson nobody has
  touched needs no frames at all. The authority sends only departures:
  hurt, knocked down, dead, frogged, cowering, chasing, talking to a player.
- **WANTED is per player.** The authority's watch (`verse/everglade/watch.coax`
  rules) keeps a crime level for each player id. Guards who see your crime
  chase you, and everyone sees them chase you. Another player is not chased
  for your crime, and the guards do not attack bystanders.
- The guard captain and guide decisions (`everglade-guard-ai.coax`,
  `everglade-guide.coax`) run on the authority only, so one decision serves
  the room and the decision model is asked once, not 16 times.
- A townsperson talking to one player (`talk`, the NPC chat at
  `/api/playtest/npc`) is shown to others as standing and talking (the talk
  clip, `PEOPLE_TALK`). The words stay private to that player.
- A late joiner gets the departures in the snapshot; the rest follows from
  the clock.

### 6.5 PvP

**Default: off.** Spells and blows pass through other players; area damage
hurts townsfolk, creatures and the town only. Helpful spells (healing,
temporary HP) do apply to other players. The realm gets a new edit row,
`pvp` ("Players can hurt each other"), off by default, for forks and
owners who want it. A duel invitation is a later phase.

### 6.6 Destructible town state and the R rebuild

- **Phase 1:** destruction stays on each page; a remote player's blast plays
  its visuals but breaks nothing on your page. (Known gap, phase 2.)
- **Phase 2:** the authority decides every break (`A e` with the piece id and
  the impulse). Every page runs the debris physics itself from that event;
  where a piece lands is cosmetic and need not match. Fire spreading is the
  authority's (which pieces burn, when they burn out).
- **Rebuild:** the 60 s regrow rule stays (a broken building stands again 60 s
  after its last damage, once no player is within 30 m). **R** becomes a
  request: the authority rebuilds the whole town if nothing broke in the last
  10 s, at most once every 30 s a room, and says "The town stands again" to
  everyone. An admin's R always works.
- Destruction is never saved. A room that empties starts whole.

### 6.7 Weather, time of day and the flock

- Weather and the day cycle are computed from the seed and the relay clock on
  every page (section 3.10). No frames.
- Your flock (the river quest's birds) is a pose flag; other pages draw a
  flock following you with their own local flocking.

### 6.8 Admin edits

- Publishing stays on `/api/playtest/world/<id>` with its owner and admin
  checks. New: after a publish, the publishing page sends `E VERSION` on the
  relay, and every page in the room refetches the world doc and applies it.
  The phase 4 server sends it itself.
- Placed props join the world for everyone within 2 s of the publish.

## 7. Persistence

| What | Survives a reload? | Where |
| --- | --- | --- |
| Name, look, class | Yes | `localStorage coaxial.everglade.character`; the account's progress when logged in |
| Position and room you were in | Yes | `localStorage coaxial.everglade.place` (`place.coax`, host action 11, already) |
| Spell bar arrangement | Yes | `localStorage coaxial.everglade.bar` (host action 12, already) |
| Quests, the flock, NPC memory of you | Yes | The realm's save slot (host action 4), and the account's progress |
| HP, spell slots, conditions | Yes, for 10 minutes | The authority keeps them by player id for 10 minutes after you leave, so a reload is not a free heal; after that, full |
| WANTED | Yes, for 5 minutes | The authority, by player id, so a reload does not clear it |
| Inventory | Yes | The account's progress; a guest's save slot |
| Spawned creatures and zones | No | They end when their effect ends |
| Destruction | No | The room starts whole when it empties |
| Admin edits | Yes | The published world doc, as today |

From phase 3 the room server keeps the 10-minute and 5-minute records, in
memory (lost on a restart; acceptable).

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

### 8.2 The Grid hand-off

- Today the galaxy and the Grid only navigate to `/worlds/everglade`. Phase 1
  keeps that.
- Phase 3: walking into the Grid's Everglade gate (`/worlds/the-grid`)
  navigates to `/worlds/everglade?from=grid&with=<friend>` when a friend you
  walked with is in Everglade, else to the lowest instance. `from=grid` spawns
  you at the South Gate facing in, under a short arrival effect. Everglade's
  HOME button returns to the Grid by its gate.

## 9. Scale and cost

### 9.1 Expected load

- Today's realms see tens of visitors a day. Plan for a launch peak of 50
  concurrent players in Everglade and design for 256 a relay instance (16
  full rooms).

### 9.2 Relay capacity

- A full room: 16 players x 15 frames/s in = 240 frames/s, fanned out to 15
  each = 3,600 sends a second of about 70 bytes. 16 rooms = 58,000 small sends
  a second, which a single-threaded C poll loop on 1 vCPU handles with
  headroom once sends no longer block (section 3.3, item 3).
- Memory: `IN_CAP` 16 KB a connection x 512 = 8 MB, plus output buffers.
  512 MiB is ample.
- The limit after that is Cloud Run's 1000 concurrency and one instance.
  Phase 4 adds instances, with the load balancer routing a room's code to
  one instance, or moves `room` to a small VM as `coder-sync-1` did.

### 9.3 Cost

All on project `openagentsgemini`, no third-party realtime service.

| Item | Estimate |
| --- | --- |
| `oa-room` kept up while any socket is open, 1 vCPU and 512 MiB. Worst case always on | about $45 to $60 a month |
| Egress: a full room is about 320 KB/s. At an average of one full room for 2 hours a day, about 70 GB a month | about $6 to $9 a month |
| HTTP presence load on Cloud SQL drops (1 Hz to 0.5 Hz per player) | no new cost |
| Phase 3 and 4 server simulation: possibly 2 vCPU | up to $50 a month more |
| Secret Manager secret for tickets | under $1 |

**Total added: under $80 a month in phases 1 and 2, under $130 with the
phase 4 server simulation.** This must fit inside the hard rule of under
$1,500 a month total GCP spend; check the current total before phase 4 and
set a budget alert on `oa-room`. Accelerators are not used.

## 10. Security

- **Validate every frame at the edge.** The relay enforces sizes, rates and
  (phase 2) identity. Clients parse frames with bounded readers (as
  `room_take` does) and ignore unknown kinds.
- **Speed and teleport checks.** Every page and the authority check each
  seat's poses: more than 12 m/s on the ground or 30 m/s in a flight form
  between samples, or a jump in position over 15 m without a warp, marks a
  violation. Pages snap the drawn body to the last good sample. From phase 2
  the authority sends the mover a correction (`A m`); three violations in a
  minute close the socket.
- **Warps** (a hash place, a fork arrival, respawn) are announced in a pose
  flag, at most once every 5 s.
- **Casts** are intents (phase 2): the authority refuses an unavailable slot,
  a spent action, an aim out of range or out of sight, and more than 4 casts
  a second.
- **Entities:** at most 16 owned entities a player, 128 a room; a spawn past
  the cap is refused.
- **Abuse:** chat limits as today; names filtered (section 4.3); mute, block,
  report and admin kick; automation isolated in `pair-` rooms and kept out of
  analytics (migration 0074); joins limited per key and per IP at the load
  balancer (Cloud Armor rate rule on `/api/room/relay/`).
- **Host trust** (phases 1 and 2): see section 3.1. No award, XP or anything
  of value is ever granted on a room host's word.
- **Privacy:** frames carry player ids and names only, never keys or emails.
  The relay logs counts, not frame contents.

## 11. Testing

### 11.1 Deterministic multi-client tests in Coax

- **Relay:** `coax test --library coax/web/room/room.coax`: 16 seats with
  two-character seats, `y` with the clock, `@SS` addressing, authority
  election and hand-off on leave and on `yield`, rate-limit drops, a full
  queue closing the slowest connection, tickets accepted and refused.
- **Two Everglades in one test** (a new `everglade-net.coax` with
  `#test`s, as `everglade-room.coax`'s room test does): two `Everglade`
  states and a relay state in one process. Frames from one page's outbox go
  through `rm_heard`/`rm_relay_next` into the other's inbox. Checks:
  - B draws A's Ranger where A stands, with A's clip, within one frame of the
    pose arriving, and interpolation never steps backward on jittered input.
  - A's Conjure Animals pack appears on B's page at the same place, follows
    A's updates and leaves on `D` and on A's `l`.
  - (Phase 2) A breaks a wall; B's wall is broken. A is WANTED; B is not, and
    B sees guards chase A. The host leaves; B becomes host and the chase and
    broken wall continue.
  - The folk clock with a 4-second skew between pages puts a townsperson in
    the same spot after clock sync.

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
  4. A breaks a crate (phase 2); B's export reports it broken.
  5. Neither session appears in real players' rooms or in analytics.

### 11.3 Load tests

- `coax/web/room/load.coax` gains an `everglade` mode: N simulated players
  (default 64, up to 256) join `pair-load-<n>` rooms in 16s, send `P` at
  15 Hz with random walks, a `C` every 5 s and an `S`/`D` pair every 20 s.
- It passes when every socket opens, the server-side fan-out time is under
  50 ms at the 95th percentile, no frame is dropped below the rate limit,
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

## 13. Phased build order

### Phase 1: everyone sees everyone, and everything they cast (today)

Build:

- The relay changes in section 3.3, items 1 to 6; deploy `oa-room` with
  concurrency 1000 and 512 MiB.
- `everglade5e-face.c` opens the socket to the instance's relay room, sends
  `H`, `P`, `C`, `S`, `D` and `V`, and hands frames to a new
  `everglade-net.coax`.
- `everglade-net.coax`: the sample rings, interpolation (section 3.8), clock
  sync (3.10), the phase 1 identity check (4.4), and the remote entities.
- Remote players drawn as skinned Rangers with clips, looks and nameplates
  (section 5); the cone figure is deleted from `everglade-room.coax`.
- Casts' visuals, spawned creatures and zones replicate from their caster
  (sections 6.2 and 6.3, phase 1 rules).
- The folk clock and weather read the relay clock, so townsfolk stand in the
  same places for everyone.
- The `[5E] ` hidden chat lines stop; `V` carries vitals.
- The swap to `/worlds/everglade` and the 301 from `/worlds/everglade-5e`
  (section 12, steps 1 to 3 and 6).
- `GET /api/playtest/rooms/everglade` and instance joining (section 3.4).

Acceptance:

1. Two browsers, one a fresh guest, open `/worlds/everglade` and land in the
   same instance. Each sees the other as a skinned Ranger with a name,
   walking, running, jumping and swinging with the right clips. No cone is
   drawn anywhere.
2. `presence-pair --socket` shows a move on the other screen within 300 ms at
   the 95th percentile on staging and production.
3. A casts Meteor Swarm and Conjure Animals. B sees the meteors fall at the
   same spot and the pack appear, move and vanish when A's concentration ends
   or A leaves.
4. With the two machines' clocks set 4 s apart, the townsfolk stand in the
   same places on both screens.
5. A phone holds 60 FPS with 5 remote bodies in view.
6. The room load test passes with 64 simulated players.
7. `/worlds/everglade-5e` redirects; a player's kept place and bar from
   either slot come back.
8. With the socket blocked, the page still shows others from HTTP presence,
   as Rangers.

Known gaps after phase 1: townsfolk HP, the watch, WANTED and destruction are
still each page's own.

### Phase 2: one world per room (1 to 3 days after phase 1)

Build: relay items 7 to 9 (addressing, authority, tickets); the room host in
`everglade-net.coax` (section 3.1); authority frames (3.6); townsfolk, watch,
WANTED per player, creatures, zones, destruction, fire and rebuild on the
authority (section 6); late-join snapshots; host migration; `E VERSION`
edit refresh; PvP off with the `pvp` row; names filter, block-hides-body,
report and kick.

Acceptance:

1. A hits a guard; B sees the guard's HP drop by the same number, and both
   see the same guard chase A. B is not WANTED.
2. A breaks a house; B sees it broken. R by either rebuilds it for both,
   under the 10 s and 30 s rules.
3. A late joiner C sees the broken house, the hurt townsfolk, A's pack and
   the chase within 1 s of joining.
4. The host closes its tab: within 3 s another page becomes host and the
   chase, the pack and the broken house carry on. Switching the host's tab
   to the background hands authority over before the world slows.
5. A forged hello claiming another player's id is refused at join.
6. An admin's placed bench appears for everyone within 2 s of publishing.

### Phase 3: the rules on the server, and the Grid gate

Build: `everglade5e-rules.coax` linked into `room`; the server takes intents
and owns all rolls and HP; the 10-minute vitals and 5-minute WANTED records;
the Grid gate hand-off and INVITE (section 8); the CHARACTER picker.

Acceptance: no client can change any HP or roll (a patched client's `A e` is
ignored by every page because only the server's seat may send it); a reload
keeps HP and WANTED; walking into the Grid's gate lands you in your friend's
instance.

### Phase 4: the whole world on the server, and scale

Build: Everglade's headless step (townsfolk, watch, zones, destruction) in
the room server; clients become viewers plus their own movement; the frames
mapped onto `realm.v1`; more than one `oa-room` instance with room-to-instance
routing; the townsfolk rig as a body choice.

Acceptance: 256 simulated players across 16 rooms with server-side worlds
within the added-cost estimate; no room host exists; the `realm.v1` protocol
conformance tests pass for the frames.

### Phase 5: a persistent world

The campaign creator's West Marches mode: expeditions, characters that keep
level and gear, a world that remembers. Out of scope here; it builds on
phases 3 and 4.

## 14. Open questions for the owner

1. **Instance size.** 16 players an instance, matching the room cap today.
   Larger (32) means busier streets and twice the bandwidth. Keep 16?
2. **PvP.** Off by default with a `pvp` row for forks and owners. Agreed?
3. **Edits made on `everglade-5e` since its split.** Copy them onto
   `everglade` before the swap, or drop them?
4. **The old Everglade page.** Remove it two weeks after the swap, or keep it
   unlisted at `/worlds/everglade-classic`?
5. **Room host trust in phases 1 and 2.** A player's page simulates the shared
   town until phase 3 and 4. Acceptable for now, given nothing of value is at
   stake?
6. **Guest names.** Generated two-word names (`Mossy Heron`) instead of
   `Guest ABCD`, and guests may pick their own through the filter. Agreed?
7. **Everyone looks like the Ranger** in phase 1, told apart by tints. Is the
   townsfolk rig (male and female bodies) needed earlier than phase 4?
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
  with migration (One-Shot's pattern, this spec's phases 1 and 2), nor a
  deterministic baseline (routines from the clock) with authority deltas
  only, nor interest limits for non-player characters.
- **Chapter 58, Everglade.** It says others are seen; it does not say that
  the townsfolk, the watch and destruction are each page's own.
- **The Verse protocol, section 6.3.** It lists Verse multiplayer as a 60 Hz
  authoritative tick. This spec uses 15 Hz player poses and 10 Hz world
  deltas, with authority moving from a room host to the server; the tick
  profile in `sim.v1` should say so. Section 10.8's `realm.v1` remains the
  target wire; the text frames in section 3.6 are its bootstrap.
- **`docs/verse/everglade.md`.** "Hostility is your page's alone" (line 789)
  and "nothing broken is saved or shared" (line 661) become untrue in
  phase 2; update them with that phase.
