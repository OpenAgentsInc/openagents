# Shared physics for several players in one small scene: a staged plan

Date: 2026-11-17. Scope: gamebuilder.org's meadow (48 boxes, a ball, up to 8
players who knock them around by running into them), on the OpenAgents
Metaverse Engine. This follows the owner's reading notes,
[physics.md](physics.md). It is a design, with no code. Lines marked
**Mine** are my inference, not a source's claim.

## Recommendation

Ship Stage 1 first: **the room steps the physics, the pages draw it.**

- One authoritative room steps all bodies at 60 Hz with the engine's rigid
  simulation. Players are kinematic movers inside that step.
- Each page predicts its own player only, with the engine's existing
  `NetPredict`. Boxes and the ball are drawn from 60 Hz snapshots, 50 ms
  behind the room's clock.
- A box you run into starts moving about one round trip plus 75 ms after you
  touch it. Under 45 ms of round trip that is below the 120 ms where Mauve's
  subjects first noticed lag. Above it, pushing feels heavy. Stage 2 fixes
  that.
- Stage 1 does **not** predict boxes, re-simulate anything on the page,
  compress against baselines, or need two machines to compute the same
  floats.

Orbit League v2 already does this with its ball, so most of it exists. Rocket
League looked at this design and rejected it (see "How a remote box is
drawn"). It is a first stage, not the end.

## The stages

| Stage | What ships | Engine has | Engine must add | Page and room do |
| --- | --- | --- | --- | --- |
| 0 (in progress) | A room-ready rigid step | `rigid.coax`, `contact.coax`, `narrow.coax`, `broad.coax` (pairs in sorted order), per-body sleep | Stable ordering with record and replay, the consequential-body snapshot, per-body history with rewind and re-simulate, several kinematic movers a step | Nothing yet |
| 1 | Room authority, boxes interpolated | `profile.coax` (authority, 60 Hz beat), `predict.coax`, `interp.coax`, `budget.coax`, `spawn.coax`, `transport.coax` (datagrams of 1,200 bytes) | A body-state codec (below), a body ring that blends quaternions (`NetState` holds a yaw only), the jitter floor as a realm parameter (`NET_LOW` is fixed at 100 ms) | A room module like `metaverse/orbit-league/server/room.coax`; the page joins, sends inputs, draws rings |
| 2 | Your own pushing is immediate | `rewind.coax` bounds, `NetCorrect` easing for your body | Predicted island selection, compare-and-re-run against history, per-body visual error offsets with rotation | The page steps its player's island; the room is unchanged except two more fields per row |
| 3 | Predict everything, cheaper wire | `replication.coax` baselines and acks | Delta rows, input-buffer throttle, room built as the same wasm as the page (optional) | Tuning |

## Who is authoritative, and why

The room. Clients send inputs and nothing else.

- Rocket League, the best-known shipped game about shared rigid bodies, is
  "100% server authoritative" with "Input delay is not an option" (Cone,
  2018, slides).
- Mauve et al. (2004) show why a peer that owns a body is a poor fit here:
  with dead reckoning "only the controller may issue operations that change
  the state of an object", so a second player touching the same box takes a
  detour through the first. Two players on one box is this scene's normal
  case.
- The engine already runs this trust model twice (`NET_TRUST_AUTHORITY`:
  Everglade at 30 Hz, Orbit League v2 at 60 Hz with two 120 Hz substeps).

A relay room with one page as host is less code, but it gives the host zero
latency, everyone else two hops, and trusts a browser. Do not build it.

## Tick rate and send rate

| Thing | Rate | Why |
| --- | --- | --- |
| Room physics step | 60 Hz fixed, accumulator, at most 4 catch-up steps a frame | Fiedler, "Fix Your Timestep" (2004): only ever step by `dt`, carry the remainder, clamp the catch-up. `NET_BEAT_HZ` is 60 |
| Inputs, page to room | One per step, each datagram repeating up to 6 unacknowledged | `transport.coax` does this today; Fiedler ("Deterministic Lockstep") and Overwatch both resend unacked inputs instead of waiting for a retransmit |
| Snapshots, room to page | 60 Hz in Stage 1, 30 Hz from Stage 2 | Fiedler ("Snapshot Interpolation", 2014): the buffer must survive two lost packets, which costs 350 ms at 10 a second, 150 ms at 30 and 85 ms at 60. In Stage 1 that delay is the push latency, so buy it down with send rate |

Rocket League steps at 120 Hz to keep penetrations small (Cone). **Mine:**
60 Hz is enough for a person running at 5 m/s into a half-metre box (8 cm a
step). Add a second substep before raising the tick.

## What goes on the wire

Absolute rows. A datagram must decode alone (`transport.coax`), so Stage 1 has
no baselines. Quantisation follows Fiedler, "Snapshot Compression" (2015).

| Field | Bits | Encoding |
| --- | --- | --- |
| Body id | 10 | 0 to 1023 (`RIGID_BODIES` is 1,024 since 2026-11-17) |
| Position | 46 | x and z in ±64 m at 1/512 m (16 bits each), y in 0 to 32 m (14 bits). Fiedler uses 512 values a metre, about 2 mm |
| Orientation | 29 | Smallest three: "2 + 9 + 9 + 9 = 29 bits per-orientation (down from 128 bits)" |
| Linear velocity | 33 | ±32 m/s, 1/32 m/s steps, 11 bits a component (Fiedler's bounds) |
| **Moving body, Stage 1** | **118** | 14.75 bytes |
| Angular velocity (Stage 2, predicted bodies only) | 33 | As linear, in rad/s |
| Finer orientation (Stage 2, predicted bodies only) | +18 | 15 bits a component. Fiedler needed this once both sides simulate |
| Player row | 100 | seat 3, position 46, yaw 10, velocity 33, flags 8 |
| Header | 48 | tick 16, your newest input stepped 16, counts and flags 16 |

A body at rest is not sent each tick. It is sent when it falls asleep,
repeated until acknowledged (as in Fiedler's "State Synchronization"), then
about every 2 s. The ±64 m bound is **mine**: I did not find the grid
square's size in metres. The room respawns a body that leaves the bound.

### Bandwidth, 48 boxes, 1 ball, 8 players

| Case | Bytes a snapshot | At 60 Hz | At 30 Hz |
| --- | --- | --- | --- |
| Nothing moving | 106 | 6.4 KB/s | 3.2 KB/s |
| 6 bodies moving (typical) | 192 | 11.5 KB/s | 5.8 KB/s |
| All 49 moving (worst) | 804 | 48.2 KB/s (386 kbit/s) | 24.1 KB/s |

Add about 40 bytes a datagram for QUIC and UDP. Uplink is about 2 KB/s. The
worst case fits one 1,200 byte datagram. For scale: Fiedler's 901 cubes cost
17.4 Mbit/s raw against a 256 kbit/s target. This scene is 18 times smaller
and needs none of his delta work yet.

## Priority when bodies outnumber the budget

Cap each seat at 32 KB/s in Stage 1 (16 KB/s from Stage 2). Use `budget.coax`
as is: it ages starved rows upward and halves a congested link, which is
Fiedler's priority accumulator.

Weights, **mine**: touching the recipient's player 100; within 3 m 20;
newly asleep and unacknowledged 20; moving 5; asleep 1. At the cap with all
49 moving, each body still gets about 35 rows a second. No interest grid is
needed: every body is relevant to every player.

## How a remote box is drawn

- A ring of the last 8 states per body. Position is interpolated linearly,
  orientation by slerp. Fiedler: "Linear interpolation is good enough at
  60HZ"; at 30 Hz use the velocity for a Hermite blend.
- Delay: 3 send intervals. 50 ms at 60 Hz, 100 ms at 30 Hz. The existing
  jitter buffer raises it toward 300 ms on a bad link.
- Extrapolation: at most 50 ms past the newest state, then hold. The engine's
  default is 150 ms (`NET_AHEAD`), tuned for walking characters. Fiedler:
  "extrapolation doesn't work very well for rigid bodies because their motion
  is non-linear and unpredictable." His cubes went through the floor at 200 ms.
- A teleport (a chat edit, a respawn) carries a generation bit and is never
  blended.

In Stage 1 your predicted player collides with boxes where they are drawn, as
Orbit League's car does with its ball. Cone's slides list this exact design
("Client predicts his vehicle", "Client interpolates ball") and its flaw:
"Client's vehicle and the ball exist in two different timelines". Rocket
League rejected it. **Mine:** it is acceptable here as a first stage because
a pushed box is slower and matters less than a struck ball, and every part of
it is reused by Stage 2.

## Stage 2: your own pushing feels immediate

Epic documents this mix: "Resimulation can be used on Actors and Pawns while
other Actors run Predictive Interpolation at the same time." Rocket League
goes further ("Client predicts everything") and pays for it: "200ms ping,
120hz = 24 correction frames".

1. **What is predicted.** Your mover, plus every awake body reachable from it
   through contacts: the island. Cap it at 12 bodies and 3 contacts deep.
   Bodies past the cap stay on the interpolated timeline and act as kinematic
   obstacles. This is the notes' "whole interacting island" rule, bounded.
2. **Entering.** A body joins the island the step your mover or an island
   body touches it. It starts from its drawn pose and keeps a visual offset,
   so nothing pops.
3. **Checking.** Each snapshot names its tick and your newest stepped input.
   The page compares the room's rows for island bodies with its own history
   at that tick. Past 2 cm, 0.1 m/s (Orbit League's thresholds) or 3 degrees
   it restores the island from the room's rows and re-runs its kept inputs.
   Cone: "Revert all physics actors to that frame in history", then "Run
   multiple physics frames to catch up". At a 100 ms round trip that is
   about 8 steps of at most 13 bodies.
4. **Smoothing.** Never smooth the simulation. Fiedler: "I recommend against
   trying to apply some smoothing between the state update and the current
   state at the simulation level." Snap the physics, keep a per-body visual
   error, and shrink it each frame: by 0.95 when under 25 cm, 0.85 when over
   1 m, in between by a blend (his numbers). Past 1.5 m take it at once
   (`NET_SNAP`).
5. **Leaving.** A body leaves when it sleeps, or 0.5 s after it last touched
   the island. Its drawing eases from your time back to the delayed timeline
   over 0.2 s.
6. **Other players.** Their movers are not predictable. Cone: prediction
   "Works well with ball (predictable)", "Not as well with cars
   (unpredictable)". Run a remote mover on its last velocity for 50 ms, then
   hold.

## Two players pushing one box from opposite sides

In the room it is one step with two movers and one body, so there is one
answer. Movers must not be infinitely strong, or the box is crushed and the
solver jitters. **Mine:** a mover pushes with a capped impulse a step (start
near 400 N times dt) and is stopped by a body it cannot move that step. The
box stalls and both players stop against it. Contacts are visited in stable
order, so a replay matches.

On the pages, each player would predict the box moving away and be corrected
every snapshot. So a body a remote mover also touches is **contested**: the
room sets a bit on its row and the page moves it from the island to the
interpolated timeline. Both see the room's answer about 100 ms late and
nobody rubber-bands. **Mine**, after Epic's warning that overwriting local
changes "does not handle interactions gracefully".

## Joins and late arrivals

1. The seat ticket and hello are as Everglade's.
2. The room sends the block list (sizes, masses) and one keyframe of every
   body, asleep or not, on the reliable session channel: about 700 bytes.
3. Datagrams start. The page draws a body once its ring holds two states.
4. A block placed or removed by chat is a reliable spawn or despawn
   (`spawn.coax`); its updates wait until the spawn is heard.
5. A dropped socket holds the seat 15 s (`LIFECYCLE_HOLD_MS`); its mover
   stays as a still obstacle.

## What determinism each stage needs

| Stage | Needs | Does not need |
| --- | --- | --- |
| 1 | A fixed step. Record and replay on the room's own binary, for debugging | Any agreement between machines. Only the room computes physics |
| 2 | The same step, constants and contact order on page and room, so re-runs are rare | Bit equality. A difference is a correction, not a desync |
| 3 | Bit equality only if rows are replaced by inputs and a digest. I would not | |

WebAssembly arithmetic is the same in every browser, and the build passes
`-ffp-contract=off -fno-fast-math`, so pages agree with each other. The risk
is a page against a native room. `docs/metaverse/engine-core.md` records that
macOS's libm and wasi-libc round sine and cosine differently, which is why
the engine has `math_sin` and `math_cos`. **Mine:** `rigid_exp` in
`rigid.coax` calls `sin` and `cos` directly; move it to the engine's own
before Stage 2 and add this scene to the native-against-wasm replay check.
Running the room as the same wasm removes the question.

Jolt and Rapier agree on what it takes. Jolt: a build option that is
"approximately 8% slower", the same calls in the same order, no contraction,
its own `Sin` and `Cos`; verified on WASM. Rapier: `enhanced-determinism`,
strict IEEE 754-2008 targets, no `simd`, its own math functions. Fiedler
("Deterministic Lockstep") warns of "differences in floating point behavior
between compilers, OS's and even instruction sets". No shipped reference
here sends inputs only. They all correct.

## Cheating, in brief

Pages send numbered keys and a heading, never a body's state. The room steps
each seat's inputs on its own clock (`NET_FAIR_EARNED`). Cone: the input
buffer "Eliminates some cheats (speed, jitter)". Stage 2's prediction changes
what a page draws and nothing the room decides.

## What to measure

| Measure | How | Target |
| --- | --- | --- |
| Push to motion: your mover touches a box, the box's drawn pose first moves | Two real pages, timestamps in the page | Stage 1: round trip + 75 ms. Stage 2: one frame |
| Remote draw error against the room's rows | The `everglade2-drift` harness, shaped to 80 and 250 ms, 1% and 5% loss, on WebTransport | 95th percentile under 10 cm at 80 ms and 1% |
| Corrections of island bodies over 5 cm, a second | Same harness | Under 1 a second |
| Step cost of this scene, native and wasm, desktop and phone | Physics Lab | Under 2 ms at the 95th percentile. The Lab's 128-box pile took 22.5 ms; Stage 2 multiplies step cost by 8 |
| Bytes a second a seat, rows starved | `?net` overlay | Under the cap, no row older than 100 ms while moving |
| Native against wasm digest of a recorded session | Record and replay | Equal, or the first differing step named |

## Later

- **Aura Projection** (Brown, Ushaw, Morgan, 2019) is for when one meadow
  becomes many rooms: a body is handed to the next server before it crosses,
  so one owner simulates a hit across the border. `grid.coax` is the start.
- **Kale and Kry** (2024) split one large constraint graph across workers and
  blend the overlap: for a pile too big for one core. It is approximate, so
  it is not an authority protocol.

## Where the notes are weaker than they sound

1. **Local lag.** The notes list delaying your own action as a tool. Mauve's
   numbers (19 subjects, a simple spaceship game): nobody noticed 120 ms or
   less, the average noticed 160 ms, the literature's safe value is 80 to
   100 ms. Rocket League refuses it for movement. Use it only as the room's
   input buffer.
2. **"Predict the interacting group" is cited to Kale and Kry.** That paper
   is about parallel simulation, not client prediction. The rule is right;
   its support is Cone's "Revert all physics actors" and Epic's mixed modes.
3. **Dead reckoning.** For boxes the relevant result is Fiedler's, and it is
   negative. Mauve adds that dead reckoning "cannot guarantee correctness".
4. **Epic.** The quote about mixing modes is accurate. The page gives no
   thresholds and says quality degrades with latency and speed.
5. **State Synchronization as the first read.** Fiedler's demo never rewinds
   and quantises both simulations so they extrapolate alike. Read it for
   priority and smoothing, and Cone's slides for the architecture.
6. **Determinism.** The notes read as if it is nearly out of reach. Jolt and
   Rapier document it across platforms, WASM included. It is reachable, and
   not needed before Stage 3.
7. **Scale.** Every implementation source here is a session of 2 to 12
   players. None shows shared rigid bodies at MMO scale.
8. **Timelines.** I could not read the paper, so the notes' summary of it is
   unchecked.

## Sources and how far I read them

| Source | Read |
| --- | --- |
| Fiedler, [Fix Your Timestep](https://gafferongames.com/post/fix_your_timestep/) (2004), [Deterministic Lockstep](https://gafferongames.com/post/deterministic_lockstep/) (2014), [Snapshot Interpolation](https://gafferongames.com/post/snapshot_interpolation/) (2014), [Snapshot Compression](https://gafferongames.com/post/snapshot_compression/) (2015), [State Synchronization](https://gafferongames.com/post/state_synchronization/) (2015) | Whole pages, through a text extractor; quotes are as it returned them |
| Mauve, Vogel, Hilt, Effelsberg, [Local-lag and Timewarp](https://publications.cs.hhu.de/library/Mauve2004a.pdf) (2004) | Full text |
| Cone, [It IS Rocket Science! The physics and networking of Rocket League](https://media.gdcvault.com/gdc2018/presentations/Cone_Jared_It_Is_Rocket.pdf) (GDC 2018) | The slides in full. Not the spoken talk |
| Epic, [Networked Physics Overview](https://dev.epicgames.com/documentation/en-us/unreal-engine/networked-physics-overview) | Whole page, through a text extractor |
| Liu, Xu, Claypool, [A Survey and Taxonomy of Latency Compensation Techniques](https://web.cs.wpi.edu/~claypool/papers/lag-taxonomy/) (2022) | The taxonomy section of the [technical report](https://ftp.cs.wpi.edu/pub/techreports/pdf/21-06.pdf); the rest skimmed. In its map this plan is self-prediction, interpolation and incoming delay, with time warp on the page in Stage 2 |
| Jolt Physics, [Deterministic Simulation](https://jrouwe.github.io/JoltPhysics/); Rapier, [Determinism](https://rapier.rs/docs/user_guides/rust/determinism/) | Those sections, through a text extractor |
| Ford, [Overwatch Gameplay Architecture and Netcode](https://gdcvault.com/play/1024001) (GDC 2017) | Not read. A [secondary summary](https://edgegap.com/blog/game-backend-deep-dive-overwatch-2016-netcode-architecture-rollback) only |
| Savery and Graham, [Timelines](https://link.springer.com/article/10.1007/s00530-012-0271-3) (2013) | Not reached: paywalled, no open copy found |
| Brown, Ushaw, Morgan, [Aura Projection](https://doi.org/10.1145/3306131.3317021) (2019); Kale and Kry, [Distributed Simulation of Large Multi-body Systems](https://arxiv.org/abs/2403.17261) (2024) | Abstracts only |
