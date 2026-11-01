# Aethyr engine core

The primitives every realm shares, in [`aethyr/`](../../aethyr/README.md)
(GitHub #12609, the verse project's milestone "Engine core"). Each has its own
checks; where it decides behaviour, `coax mutate` leaves no survivors. The
plan they follow is [docs/research/game-engine-systems.md](../research/game-engine-systems.md),
sections A and D, and coax/ROADMAP.md R12, R13 and R16.

## Loop (`aethyr/loop.coax`)

One clock owns time. A realm hands it each frame's time once; it says how
many fixed steps to run and how far the frame lies between the last two.

- Time is whole nanoseconds, so the steps a run takes depend only on the
  frame times fed in, not on float rounding in a running total.
- A frame counts for at most `max_frame_ns` (0.25 s by default), at most
  `max_steps` (8) run in one frame, and a backlog past that is dropped and
  counted in `dropped_ns`: a stall never makes the next frame slower.
  Non-number and infinite frame times count as none.
- `loop_alpha` is what drawing mixes the last two steps by.

The player controller (`verse/player.coax`, every walking realm) steps on it
at 1/64 s, and Orbit League at 1/120 s. Checks: a run and jump passes through
the same states, bit for bit, at 30, 60, 120 and 144 FPS; drawn at 60 FPS, a
jump's feet change their per-frame move by at most 0.008 m (gravity alone is
0.0067 m; drawing the last step instead lurches by 0.077 m) and its camera by
at most 0.014 m (0.062 m).

## Space (`aethyr/space.coax`)

A position's type is its space: `WorldPoint` (metres, where physics puts
things), `LocalPoint` (metres in a `Placement`: an origin and a yaw),
`ViewPoint` (metres from a `Lens`: x right, y up, z ahead), `ScreenPoint`
(CSS pixels) and `GridPoint` (the Grid's drawing units, two metres each).
They move between spaces only through named functions (`local_to_world`,
`world_to_grid`, `world_to_view`, `view_to_screen` and their inverses), and
distances are R12 quantities, so metres and grid units don't mix
(`grid_metres` converts, exactly). The lens and projection are the Grid's
(`newland.coax` `cam`/`spot`); a point nearer than the lens's near plane is
not drawn. `world_to_grid` scales heights as it scales the ground, so feet
are drawn at their physics height.

Mixing spaces is a compile error: `coax/web/tool.sh aethyr/refusals.coax`
compiles each fixture in `aethyr/tests/spaces/` and checks the code it is
refused with (E0317 a world point given for a view point or a grid point for
a world one, E0325 a local point stored as a world one, E0822 metres plus
grid units), against a control that must compile. Mutation: 78 of 78 killed
(`space.coax`), 47 of 47 (`refusals.coax`).

## State (`coax/web/defaults.py`, `coax/web/state-defaults.test.ts`)

Every realm's state gets its declared field defaults by construction, natively
(`s: [1] T`) and in the browser, where the host hands an engine zeroed memory:
the build runs `defaults.py` over the generated C, which exports
`coaxial_state_init` for the host to call on a fresh state and defaults the
face's globals (the player, the thumbs) in a constructor. The check boots
every engine `build.sh` makes, and the Grid, from zeroed memory and compares
each defaulted part with native `coax_init`, byte for byte. It is sharp: the
same state left zeroed must differ, or the realm fails. 16 realm states pass
(Belt, Dead Gears, Everglade, Fairy Forest, Garden, Ice Cream Man, Memorial,
Orbit League, Hold the Stair, the villa and the pages with state defaults);
a realm with no defaults has nothing to apply. `aethyr/loop.coax`'s clock
also takes its default step, frame clamp and step cap on its first frame if it was zeroed, so it never freezes.

## Input (`aethyr/input.coax`)

Keys, touch and pads become one `Intent` a frame (`input_keys`,
`input_stick`), with one run rule (`INPUT_RUN_BY_DEFAULT`: moving runs,
Shift walks; a stick past the dead zone runs on a full push) and one dead
zone (0.2). The steps read it through an `InputBuffer`:

- Levels (the move axes, run) are what is held when a step samples.
- Presses (jump, act) are counted when they happen, and the first step that
  samples after a press takes it: a frame with no step keeps it, and a frame
  with two steps gives it to the first. At most four wait; more are counted
  as lost.
- Look and zoom add up between steps, and a frame's steps split them, so a
  drag turns by the same total at any frame rate.
- Values that aren't numbers, and axes past full travel, are refused or
  clamped and counted (`mended`). `input_blur` releases every level at once.

The walking controller (`verse/player.coax`) takes its jump from the first
step after the press; its look still turns the camera at once until the
camera moves to the steps (ec-camera). Checks: every press goes to the step a
separate model names, over 64 random runs of 200 frames with frames of zero
to eight steps; a keyboard session and the same touch session give the same
`Intent` and the same player states; a press in a frame with no step jumps
on the next. Mutation: 126 of 126 killed (`input.coax`).

## Query (`aethyr/query.coax`)

A realm's colliders are one `Colliders` value in world space: a flat ground,
boxes, capsules and floors (`verse/stand.coax`). Four queries read it:

- `query_ground` (and `query_stand` for feet): the highest surface at or
  below a height, its slope, whether it is too steep (`QUERY_STEEP`, 55
  degrees) and which collider it is. The step height (0.55 m) and the floor
  pad (0.15 m) are defined here once.
- `query_ray`: the first hit in [0, far], with its point and outward normal.
- `query_overlap`: the colliders a sphere overlaps, in collider order.
- `query_sweep`: a moving sphere's first touch in [0, 1], as a ray against
  the colliders grown by its radius (a box grown by r is three boxes, each
  grown along one axis, and twelve edge capsules).

Every shape is solid, so a ray or sweep that starts inside one hits it at
t = 0, and ties go to the earlier collider. The walking controller's
`floor_under` is one `query_stand` call, so every walking realm asks the same
ground question. Checks: rays and sweeps agree with an oracle that marches by
containment (`aethyr/tests/query-oracle.coax`) over 48 random scenes each, to
1 mm and on the collider; overlaps agree with separate distance code; the
walker's run, jump onto a 0.5 m ledge, walk across and walk off give the same
states whether a realm passes walls alone or walls among far bodies and
floors. Mutation: 312 of 312 killed; an independent review found the float32
inputs that kill the last five (the capsule rim, roots at exactly `t = 0`,
and a face-edge tie). Heightfields, rays against floors and the
realms' own `ground_at` copies move with ec-migrate; the camera's
`camera_room` moves with ec-camera.

## Entities (`aethyr/entity.coax`)

An entity has a handle, a 20-bit slot and a 12-bit generation, for use
inside a step, and an `EntityId` from the store's counter, for saves, the
network and edits. A despawn raises the slot's generation, so a stale handle
never reaches the slot's next owner, and a slot whose generation would wrap
is retired and counted. `entity_spawn` and `entity_despawn` are commands:
`entity_flush` applies the despawns in slot order, then the spawns in the
order they were asked for, so every system in a step sees the same
entities. The realm owns the storage (an `EntityStore` is views), and
`entity_digest` and `entity_copy` give replay and saves one number and one
copy. Checks: after every flush of 24 seeded sessions, the store matches a
plain list of IDs and values (`aethyr/tests/entity-oracle.coax`);
despawning while walking visits each live entity once; a session saved
mid-way and loaded, and the same session replayed from nothing, end with the
same digest and IDs. Mutation: 197 of 197 killed. Dead Gears'
`verse/deadgears/ecs.coax` moves onto it with ec-migrate.

## Replay (`aethyr/replay.coax`, `rand.coax`, `math.coax`)

Every external event a session accepts is one `ReplayEvent`: its apply tick,
source (controls, a model's answer, a network update), request or sequence
number, epoch and payload. `replay_offer` is the gate: it refuses an event
during a replay, an unknown source or unasked answer, an event from another
epoch, a model request no longer open, or a stream number older than the
last accepted (stale), and an answered request or a stream's last number
again (duplicate), counting each; model answers are accepted in any order; it stamps an accepted
event with the next step. Live and replay apply events the same way:
`replay_take` before each step. `replay_ask` numbers requests in both modes
and says to send them only live. A record starts with the constructed
state's digest (`replay_play` refuses another), keeps a state digest every
step, and reports the first step that differs. `aethyr/rand.coax` draws by
seed, stream and counter. `aethyr/math.coax` computes sine, cosine and
powers from exact IEEE operations: with the C library's, a recorded walk
parted natively and in wasm after 105 steps; `verse/player.coax`'s step uses
them now. Checks: live sessions with late, repeated and out-of-order
answers and updates replay to the same digest at every step with no request
sent; a planted fault is found at its step; `coax/web/replay-hosts.test.ts`
gives the same session digests for a recorded walk natively and in wasm.
Mutation: 321 of 321 (`replay.coax`), 50 of 50 (`rand.coax`), 154 of 154
(`math.coax`).

## The Grid on the engine core (`verse/gridwalk.coax`)

The Grid (`/realms/the-grid`) is the first realm whose simulation is one Coax
session on the engine core. `GridWalk` holds you (`verse/player.coax`), the
phone's thumbs (`verse/sticks.coax`, moved out of `verse/grid.coax`), the
input buffer and the session's record. The page's C face calls
`gridwalk_frame` once a frame with the keys held, the drag, arrow, tilt and
wheel amounts and the frame's time, passes touches to `gridwalk_touch`, and
reads back where to draw you and the camera. It no longer clamps the frame
time or steps the player itself.

- Each frame's controls are one `Intent` (the stick past its dead zone, else
  the keys) and one `REPLAY_ACTIONS` event. Each fixed step takes its events
  from the record, feeds the input buffer and samples it, so a live session
  and its replay step the same way, and each step leaves a digest.
- Look and zoom are applied on the steps and split among a frame's steps;
  the camera's turn and tilt are drawn between the last two steps.
- A record holds 1,024 frames and 2,048 steps. When either is full, a new
  record starts from the state the session has then, with the frames still
  waiting for a step in its input buffer.

Checks: `verse/gridwalk.coax` (the keyboard and the touch stick give the
same digests; a tap in a frame with no step jumps on the next; look and zoom
total the same at 30, 60 and 144 FPS; a recorded session replays to the same
digest at every step and a changed frame is found at its step; full records
start new ones that replay), and `coax/web/replay-hosts.test.ts`: a 1,000-frame
Grid session (keys, Shift, Space, arrows, a drag, the wheel, the stick, a
full push, a look finger, a pinch and taps, at 60, 120 and 144 FPS with a
stall) gives the same digest at every one of its 673 steps natively and in
wasm.
