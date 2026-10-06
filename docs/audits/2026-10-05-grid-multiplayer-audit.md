# Grid multiplayer audit: a shared world with portals for 10 to 20 players

Date: October 5, 2026. Baseline: `main` at `ce2598fb20`. Scope: what the
Grid (the OpenAgents app's bare world), its portals, the Everglade and
Lagrange 1 zones, and the authoritative chamber give a group of 10 to 20
players today, and what is missing before everyone who opens the Grid on a
phone, a desktop, or a browser sees the same players walking the same world,
reads a name over each head, and can walk through a portal into one shared
instance of each zone.

This audit complements the
[Verse engine audit](2026-10-04-verse-engine-audit.md) (V01 to V28). That
audit's remediation (V04 in #10580, V18 crowd recovery, the networking plan's
REACH convergence #10552 and shared Everglade #10553) is owned by other work
and is not re-planned here. This audit plans the player-facing multiplayer
layer that sits on top of it: the Grid as the lobby, the zones as shared
instances, and the tests that prove 20 players move smoothly.

## What exists

| Surface | Shared today | How |
| --- | --- | --- |
| The Grid on iOS and Android (`coder-mobile::verse_app`) | Positions, the ball and blocks | NIP-MV presence in world `verse-bare` on `wss://relay.openagents.com`: pose frames every 3 s moving and 5 s idle (`PublishIntervals::mobile`), capped at 54 events a minute per key; remote avatars drawn 3.3 s in the past (`crowd::DELAY` raised by the mobile profile) |
| The Grid on the desktop (`openagents-desktop` Play, `GridSurface`) | Same as the phone | Same session and physics; desktop keyboard and mouse |
| Name tags | The first eight hex characters of the pubkey, plus ` · lv n` for a key with shown XP (`verse::xp::name_tag`) | No display name: the bare-world session publishes none (`Session::start_presence`) |
| Everglade from the Grid's arch | Nothing | Presence pauses on the way through the arch; the glade is a local world; it rejoins `verse-bare` on return |
| Lagrange 1 from the Grid's arch | Nothing | Same as Everglade |
| The ritual chamber (cultists) | Combat, poses, items, quests for enrolled keys | `verse-world::service` over TLS and TCP, static enrollment, one instance per process; `openagents chamber host` and the desktop `verse_remote` client; no phone or browser client; no portal from the Grid |
| Everglade on the web (`everglade-web`, `/everglade`) | Nothing | Offline glade, no relay, no presence |
| The Grid on the web | Does not exist | `openagents-web` serves Everglade only |
| Simulated players | `grid_walkers` example: three walkers on an in-process loopback relay | For watching the desktop backdrop; no public relay, no count, no measurement |
| Load measurement | The chamber's `verse_load` and the battle harness (20 players, 40 NPCs) | Chamber only; nothing measures the Grid's presence path or a client drawing 20 remote avatars |

## Gaps

### G1: Remote players do not move smoothly

A phone publishes a pose every 3 s while moving and the viewer draws others
3.3 s in the past. At walking speed (6.4 m/s running) a player covers about
20 m between samples; the interpolation hides the jump but every remote
player is three seconds behind and corners are cut. The desktop publishes at
100 ms, but the relay admits 60 events a minute per key
(`events_per_minute_pubkey`), so a desktop that moves for more than a minute
is throttled (`rate-limited:`) and falls back to the same cadence. NIP-MV
already asks relays for a separate per-second lane for `23300` frames; the
OpenAgents relay does not give one.

**Target:** a moving player publishes at 5 Hz on every platform, the relay
admits pose kinds on their own per-second budget, and viewers draw others
about 300 ms in the past with velocity extrapolation across a late frame.
Measured: visual delay under 0.5 s and no `rate-limited:` refusals with 20
players all moving.

### G2: Twenty players is untested on the Grid

Nothing starts N simulated Grid players against a chosen relay, and no
receipt records what the phone, the desktop, or the relay do with 20 of
them: frames per second drawn with 20 remote avatars, events per second the
relay accepts, bytes per client, stale evictions. The chamber's harness does
not cover this path.

**Target:** `openagents verse walkers N` (loopback or a named relay, named
world, chosen cadence) and `openagents verse load` that joins as a viewer
and records received frames per second, delay, and gaps per player as
NDJSON, plus a frame-time probe for the native client with 20 remote
avatars. A retained receipt under `bench/verse/` per platform.

### G3: No readable name over a player's head

Tags show a pubkey prefix. A player cannot set a name and nobody can tell
who is who.

**Target:** an account display name, published as the NIP-MV entity state's
name and in a kind `0` profile for the world key, drawn over the head on
phone, desktop, and web, with the level suffix kept; a sanitized, bounded
name (length and script limits) and a block list so a name cannot be used
for abuse.

### G4: Zones are not shared instances

Everglade and Lagrange 1 are local worlds. Two players who walk through the
same arch each get their own copy and lose each other until they return.

**Target:** each zone is one NIP-MV world (`verse-everglade`,
`verse-lagrange-1`), with the Grid's session re-keyed to the zone's world
on the way through the arch, so everyone in a zone sees everyone else there,
with the same name tags and collision. The shared Everglade with
authority-owned seats and a social rules profile (#10553) remains the
authoritative successor; this slice gives presence now, over the same world
identifier, so it is not thrown away.

### G5: The cultist chamber has no door from the Grid and no phone or browser client

The chamber is the one authoritative zone and the one with the cultist
simulation, and it is reachable only from a cargo example or
`openagents chamber` with a hand-written config, a DER certificate, and a
static enrollment. There is no portal to it, it is not running anywhere, and
the mobile and web surfaces cannot connect to it (raw TLS, no client code).

**Target, in order:**

1. A public instance: `openagents chamber host` under a resident service
   with a guest admission mode (any key may join as a player, with a per-key
   and per-address budget, a population cap, and the existing refusal
   semantics), beside the static list.
2. A **RITUAL** arch on the Grid that connects the desktop's existing remote
   chamber client to that instance, with the pinned certificate and content
   digest shipped as a zone pin, and a return arch.
3. The phone joins the same instance through the same client worker
   (`verse-world::service::client`) and the shared presentation.
4. The browser joins once the chamber wire has a WebSocket transport. That
   transport is the REACH convergence in #10552 and is not duplicated here;
   the web Grid slice (G6) leaves a hook for it.

### G6: There is no Grid in the browser

`everglade-web` draws an offline glade. A browser cannot open the Grid, see
other players, or publish presence, although the relay is a WebSocket
server a browser can reach.

**Target:** a `grid-web` build of the same `GridSurface` scene with NIP-MV
presence over the browser's WebSocket, name tags, the portals, and the
same cadence and crowd rules, served at `/grid` by `openagents-web`.

### G7: Admission and abuse on a public world

Anyone can publish any frame in `verse-bare`. There is no block or mute,
no population cap, no eviction of a key that floods, and no way to see who
is present from the command line beyond `openagents verse who`.

**Target:** per-world population cap and a relay-side per-world frame budget;
client-side block and mute kept across sessions; `openagents verse who
--world` and `verse tail` reporting a world's live population, cadence, and
refusals.

## Slices and playable tests

Each slice ships behind its own issue with a test a person can run; the epic is #10590.

| Slice | Issue | Playable test |
| --- | --- | --- |
| 1. Walkers and load measurement (G2) | #10581 | `openagents verse walkers 20 --relay wss://relay.openagents.com --world verse-bare` then open the Grid on a phone: 20 players walk loops in front of the spawn; `openagents verse load --players 20` prints the receipt |
| 2. Smooth motion and the relay's pose lane (G1) | #10582 | Two phones, or a phone and the desktop, run beside each other: the remote avatar follows within half a second; the walkers at 5 Hz stay smooth |
| 3. Name tags with display names (G3) | #10583 | Set a name under **Account**; a second device reads it over the head |
| 4. Shared zones through the arches (G4) | #10584 | Two players walk through the Everglade arch and meet in the glade; the walkers can be started in `verse-everglade` |
| 5. Public chamber instance with guest admission, and the RITUAL arch (G5.1, G5.2) | #10585 | Walk through **RITUAL** on the desktop Grid and fight the cultists with another desktop; `openagents chamber status --to ritual.openagents.com:…` shows the population |
| 6. The phone in the chamber (G5.3) | #10586 | Same as slice 5 from a phone |
| 7. The Grid in the browser (G6) | #10587 | Open `/grid`, walk, and see the phone's player |
| 8. Admission and abuse controls (G7) | #10588 | Block a walker; it disappears for you and stays gone after relaunch; a 21st walker is refused by the cap |
| 9. The 20-player soak (G1, G2) | #10589 | 20 walkers plus a phone, a desktop, and a browser for 30 minutes: frame-time p95 under the device budget on each, no refusals, receipt retained. The sustained 20-player movement soak is the `battle_scale` harness and gates of #10559; `verse load --max-age-ms 1000` checks the 1-second frame-age limit at a viewer beside `verse walkers` |

Slices 1 to 4 need no new authority and land first. Slice 5 waits on
nothing but a machine to host the instance. The browser's chamber client
waits on #10552.

## Budgets for acceptance

- Remote-avatar visual delay under 0.5 s at 5 Hz publication; no
  `rate-limited:` refusals with 20 players moving.
- Native client frame-time p95 under 16.7 ms on the reference desktop and
  under 33.3 ms on the phone with 20 remote avatars and the Gym in view;
  browser under 33.3 ms.
- Relay: pose events accepted per second per world recorded; no accepted
  frame older than 1 s at the viewer.
- Receipts under `bench/verse/<date>/grid-*` with revision, relay, platform,
  player count, and the raw NDJSON.
