History: see everglade/ARCHITECTURE.md.

# Everglade multiplayer audit (#13157)

2026-11-10 to 11-11, Chicago time. The complaint: "I don't see an arrow fly in
Everglade on desktop, and it's laggy and barely responsive compared to
single-player." This audit measures Everglade II online against AAA MMO
netcode practice, names why arrows didn't fly, and records what changed.

## How it was measured

`coax/web/tool.sh everglade2-latency URL [--trials N] [--shot DIR]` opens two
headless pages, A and B, in a test room (`pair-e2-<hex>`, never a live
room). It makes A a Ranger, seats both, and then times each input to the
first frame that answers it, from the engine's own probes and not from the
pixels:

- **walk**: W down to your body drawn moving.
- **turn**: mouse travel to the camera, and your body, facing it.
- **attack**: the Attack key to the first frame that answers it.
- **loose**: the right button up to your arrow in the air (A), and to B
  drawing that arrow and playing the Attack.

It also reports the round trip, the interpolation delay, bandwidth, the
room's replies, and a clip from each page. The bad network is
`coax/web/room/net-proxy.coax` at 120 ms round trip, ±15 ms jitter and 2%
loss with TCP-style resends.

The "before" runs are against production (`openagents.com`, page from main,
room `oa-room-00042-nlp`). The "after" runs are against a local room built
from this change.

## Results

| Input to visible response | Before (production) | After, clean | After, 120 ms + 2% loss |
| --- | --- | --- | --- |
| Walk, p50 / p95 | 12-32 / 15-46 ms | 24-30 / 42-53 ms | 23-28 / 28-34 ms |
| Turn, p50 | not measured | 9-12 ms | 10-13 ms |
| Attack key, p50 | never (0 of 26 answered) | 10 ms | 10-11 ms |
| Loose to your arrow in the air, p50 | never (no arrow exists) | 10-13 ms | 8-10 ms |
| Loose to B drawing A's arrow, p50 / p95 | never | 22-56 / 26-64 ms | 142-267 / 302-448 ms |
| Loose to B playing A's Attack, p50 | never | 145-148 ms | 230-318 ms |
| Room confirms A's arrows | 0 of 12 (`r SEQ range`) | 16 of 16, 0 mm apart | 16 of 16, 0 mm apart |
| Others drawn behind (A) | 300 ms (spread 6 ms) | 100 ms | 100 ms (spread 11 ms) |

Walking was never slow: your body moves within two frames online, the same
as offline. The "laggy" feel was the bow and every attack doing nothing,
plus other players drawn 300 ms in the past.

Clips (A shoots toward B; B faces A and sees the arrow fly and stick):

- before: `before-prod-a.mp4`, `before-prod-b.mp4`
- after, clean: `after-local3-b.mp4`, `after-clean-v2-a.mp4`, `after-clean-v2-b.mp4`
- after, bad network: `after-bad-v2-a.mp4`, `after-bad-v2-b.mp4`

## The room, its host and the wire

- **Tick.** The room is authoritative at 30 Hz (`EG_HZ`,
  `coax/web/room/room-everglade.coax:58`; `WIRE_HZ`,
  `apps/openagents-web/everglade-wire.coax:108`). Thirty matches the 33 ms
  input step the pages predict with and keeps a room's whole tick at about
  0.1 ms (p50 in every shard's `/health`), so one instance holds many seats.
- **Host.** Cloud Run `oa-room` plus `oa-room-2` to `oa-room-6` in
  us-central1 with minimum instances 0. From this Mac: TCP connect 40 ms,
  WebSocket round trip 22-49 ms, `/api/room/s1/health` TTFB 130-270 ms.
- **Snapshots.** Delta rows against each seat's acknowledged rows, as
  decimal text, under interest management (`room-interest.coax`: 32 m grid,
  enter 110 m, leave 130 m, priority under a byte budget).
- **Bandwidth.** About 2.6-2.7 KB/s down at 53-55 frames/s and 0.7-0.8 KB/s
  up at 30 frames/s per player, with two players.

## Why the arrows didn't fly

1. **The production room predates the Ranger's Longbow, so the room refuses
   every shot.** A sent 8 and then 12 Attacks; every one came back
   `r SEQ range`. 9d847d9398 (2026-11-10 18:47) made a bow kit's Attack the
   Longbow's shot (`apps/openagents-web/class-kit.coax` `xk_rules`, used at
   `apps/openagents-web/everglade2-rules.coax:61`). Production's room is
   `oa-room-00042-nlp`, deployed 2026-11-10 00:30, so it still judges a
   Ranger's Attack at the shortsword's 5 ft. Pages ship through
   `coax/web/release.sh`, which only builds the room for its drift check
   (`release.sh:196-210`). The room ships through
   `coax/web/room/deploy.sh`, and nobody ran it. Filed as #13163.
2. **No arrow existed in Everglade II online.** On main a loose was a room
   cast: `e2m_loose` (`apps/openagents-web/everglade2-mouse.coax:41-47`)
   pressed the Attack through `e2k_press`
   (`apps/openagents-web/everglade2-cast.coax:154`). That played the generic
   cast look on the room's yes (`e2k_taken`, `:266`). Neither the page nor
   the room had an arrow entity, so even an accepted shot showed no arrow.
3. **Every attack waited for the room's round trip.** The cast header said
   it outright: "A press asks first and plays only once the room says yes"
   (`everglade2-cast.coax:10` on main). Online, every ability answered no
   sooner than one round trip, and never when refused.
4. **Others were drawn 300 ms in the past.** The page maps room ticks to
   its clock from the moment it heard the world header (`w_t`,
   `apps/openagents-web/everglade2-net.coax:411`), which is resent only
   when the weather changes. A room whose ticks run long, or that started
   them late, shows up as lateness that only grows. The jitter buffer
   (`everglade-net-core.coax` `net_jitter_delay`) then climbed to its
   300 ms ceiling over a 6 ms spread.

## What changed

- **Arrows the room flies** (engine: `metaverse-engine/net/projectile.coax`,
  `metaverse-engine/net/rewind.coax`, `metaverse-engine/net/shots.coax`; room:
  `coax/web/room/room-e2-arrows.coax`).
  - Your page draws your arrow the frame you let go, on the shared archer
    (`metaverse-engine/action/archery.coax`, `bow.coax`, `everglade/longbow.coax`),
    and sends `F SEQ FROM TO STRENGTH VIEW`.
  - The room flies the same arrow from the moment your page was drawing,
    against the bodies as they stood then: lag compensation, bounded to
    250 ms of age and 400 ms of view, with 24 ticks of the past kept. It
    names the arrow (`a TICK ID SEAT SEQ ...`) and says where it ended
    (`d TICK ID KIND ...`).
  - Your page matches the room's word to its own arrow by request number
    and eases out the difference (0 mm in every run).
  - Other pages draw the arrow from the room's word, at the same delay as
    its archer, so it leaves the bow they see.
  - The room's rules decide every hit.
  - `rewind.coax` is `#authority` and mutation-clean: 29 of 29 mutants
    killed.
- **Predicted casts with clean rollback**
  (`apps/openagents-web/everglade2-cast.coax`).
  - A press plays at once, cooldown and global cooldown included.
  - A refusal, or no answer within 1.5 s, takes it back with fades.
  - A late yes after a timeout plays it after all.
- **The tick map follows the states**
  (`everglade-net-core.coax` `net_map_follow`).
  - After each state the map takes 5% of the mean lateness up, so the
    buffer holds the spread and not the drift.
  - Under 120 ms and 2% loss, others are drawn 100 ms behind instead of
    300.
- **A netcode overlay** (`metaverse-engine/net/stats.coax`; Everglade II `?net`).
  - It shows RTT, tick rate and state-frame gap, interpolation delay,
    bytes and frames each way, snapshot size, and prediction error.
- **One room-side bug fixed on the way.**
  - In a room younger than 64 ticks, the arrow epoch went negative and
    arrows never flew.
  - The room's test helpers also split relay frames on `|` the way the
    relay joins them.

## Against AAA practice

| Practice | Everglade II now | Gap |
| --- | --- | --- |
| Server-authoritative fixed tick, sequenced inputs | Yes: 30 Hz, inputs queued per seat, deduplicated | |
| Your character: predict, replay unacknowledged inputs, smooth correction | Yes: 1/30 s steps, replay on each fix, eased over 0.12 s, snaps past 1.5 m | Up to one step of draw delay (#13167) |
| Others: interpolation, capped extrapolation | Yes: adaptive buffer of 100-300 ms, 8 states, 150 ms ahead at most; now free of drift | |
| Lag compensation with a bounded rewind | Yes, for arrows: 250 ms of age, 400 ms of view, 24 ticks kept | Melee and spells still judge at the present |
| Projectiles: server-simulated, predicted, matched by ID, others' from snapshots | Yes | Trunks and rocks don't stop arrows (#13166) |
| Interest management, delta-compressed and quantized snapshots, per-entity rate | AOI with hysteresis, priority budget, deltas | Decimal text, every entity every tick (#13165) |
| Unreliable state, reliable events | One WebSocket over TCP | Head-of-line blocking under loss (#13164) |
| Tested under 100-150 ms, loss and jitter | Yes: net-proxy at 120 ms, ±15 ms, 2% | |
| Abilities: predicted start, GCD and cooldown, server confirm or reject with rollback | Yes | |
| Netcode debug overlay | Yes: `?net` | |

## What ships how

The page ships with `coax/web/release.sh`. The room's arrows and the Longbow
fix reach players only when the room is deployed with
`coax/web/room/deploy.sh` (#13163). Until then, production's room refuses
every Ranger shot, and the new page draws the arrow and fades it out on the
refusal.
