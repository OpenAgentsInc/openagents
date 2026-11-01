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
- `loop_counted` is the time the last frame counted for, after the refusal,
  clamp and drop, and `loop_counts` the time a frame would count for. A realm
  moves everything outside its steps (animation, its room's timing) by it, so
  one clock clamps the whole frame. `ran` is the steps the last frame ran.

The player controller (`verse/player.coax`, every walking realm) steps on it
at 1/64 s, Orbit League at 1/120 s, and Dead Gears' rules
(`verse/deadgears/ecs.coax`), Everglade's debris (`verse/demolish.coax`) and
Hold the Stair at 1/60 s. The page faces of the haunted house, Fairy Forest,
the Garden, Everglade and the villa no longer clamp the frame to 0.1 s; they
move their realm by `verse_player_counts`, what the player's clock counts the
frame for.

No frame runs more steps than a realm's budget (8, `aethyr/budget.coax`):

| Realm | Step | Clamp | Most steps a frame |
| --- | --- | --- | --- |
| Walking realms (the player) | 1/64 s | 0.25 s | 8 |
| Orbit League | 1/120 s | 0.1 s | 8 (it was 12) |
| Dead Gears' rules | 1/60 s | 0.05 s | 3 |
| Everglade's debris | 1/60 s | 0.25 s | 6 |
| Hold the Stair | 1/60 s | 0.1 s of play times the speed | 8 (it was 64) |
| Hold the Stair in co-op | 1/60 s ticks of `speed` steps | 8 ticks | 8 |

At 8x, Hold the Stair runs at full speed down to 60 FPS; below that, and in
Orbit League below 15 FPS, the time past the cap is dropped and counted on the
clock (`stalls`, `dropped_ns`). Everglade runs two clocks a frame, the player's
and the debris', so a stalled frame there runs up to 14 steps in all. Checks: a run and jump passes through
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

## State (`coax_default_T`, `coax/web/state-defaults.test.ts`)

Every realm's state gets its declared field defaults by construction, natively
(`s: [1] T`) and in the browser, where the host hands an engine zeroed memory:
each library header declares the compiler's `coax_default_T` for its types
with defaults, and the realm's face exports `coaxial_state_init`, which calls
it on a fresh state, and calls it for the face's globals (the player, the
thumbs) in a constructor. The check boots
every engine `build.sh` makes, and the Grid, from zeroed memory and compares
each defaulted part with native `coax_init`, byte for byte. It is sharp: the
same state left zeroed must differ, or the realm fails. 17 realm states pass
(Belt, Dead Gears, Everglade, Fairy Forest, Garden, Ice Cream Man, Memorial,
Orbit League, Hold the Stair, the villa, Water Lab and the pages with state
defaults), and so do 20 face globals, compared natively after the face's
constructors run;
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
step after the press. In the realms that call `verse_player_input` once a
frame, its look still turns the camera at once; the Grid's session splits
the look over the steps, and the other realms follow with ec-migrate. Checks: every press goes to the step a
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
`camera_room` is one `follow_room` call (Camera, below).

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

## Camera (`aethyr/follow.coax`)

The follow camera behind a walker is one module. A `Follow` holds the turn
and tilt the look input sets (`follow_look`), the distance the zoom asks for
(`follow_zoom`, 1 to 20 m), the point the camera looks at with its spring
velocity, and how far out it is shown.

- `follow_track` is one step of it: the point follows the walker's head on
  a critically damped spring (time constant 0.1 s) that never overshoots,
  then the camera pulls in at once to the room it has, never nearer than
  0.35 m, or backs out by 18% of the gap each 1/60 s. Both are closed forms
  in the step's time.
- `follow_room` reads a realm's `Colliders` (chapter 7): boxes and capsules
  widened by 0.2 m stop the line out from the target. Beside a box, the box
  holds the camera in only where the line runs into the box itself. It never
  changes the colliders.
- `follow_at` and `follow_eye` give what to draw between the last two
  steps; the eye is never shown further out than the last step.
- `follow_offset` uses `aethyr/math.coax`, so a step that reads the camera's
  direction (the walker steers by it) has the same bits on every host.

Every camera answers with one `CameraShot`, an eye and the point it looks
at (`aethyr/shot.coax`, which loads only `aethyr/space.coax`): the follow
camera in third person (`follow_shot`) and first person
(`follow_first_shot`), top down (`shot_top`), the free orbit
(`shot_orbit_ray`, `shot_orbit`) and the dive (`shot_dive`: from a far shot
down to a live one in 0.75 s by default, eased at both ends, the look-at
arriving 1.3 times sooner; a time that isn't a number is a cut). Each reads
state already drawn between steps; none steps physics.

The walking controller (`verse/player.coax`) keeps its camera fields, which
the realms' C faces read. Its step no longer moves them: `camera_step` runs
`follow_track` after each player step (`step_once`), and its view modes are
the three shots. `verse/camera.coax`'s free orbit draws its shot through
`aethyr/shot.coax`. The arithmetic is the same as before (200,000 random
orbits and 100,000 random walkers gave the same bits), so the Grid's pinned
session digest and its recorded host session are unchanged. Checks:
`aethyr/follow.coax` (the spring against its closed form, room against hand
calculations for boxes, capsules, slants and the wall band, pull-in and
back-out at any step size, tracking leaves the colliders as they were, the
third- and first-person shots), `aethyr/shot.coax` (top down, the orbit's
line outside, halfway and inside, a dive that lands exactly and moves at most
3.43 m a frame over 87.78 m in 48 frames, cuts), `verse/player.coax` ("the
player is the same whether its camera steps every step, never, or twice a
step": a walk into a wall, round it and past a body with two jumps and a
turn at 60 FPS, comparing 21 fields every frame) and `verse/gridwalk.coax`
("the Grid's camera follows a walk, a run and a jump at 60 FPS without a
jump": the eye drawn each frame moves at most 0.177 m, its move changes by at
most 0.016 m between frames (measured 0.013 m), it rises every frame of a
jump's rising arc, and drawing the last step instead changes its move by more
than three times the bound). Mutation: 189 of 189 (`follow.coax`), 54 of 54
(`shot.coax`), none surviving on the changed lines of `player.coax` and
`camera.coax`. `verse/space.coax`'s `Shot`, the homepage's dive into the
Grid, Ice Cream Man's boom and `SpaceCam` move onto `CameraShot` with
ec-migrate.

## Budgets (`aethyr/budget.coax`)

A realm declares what one frame may use, a `FrameBudget`: fixed steps (8,
the clock's cap), vertices, draw records and the bytes the page host holds
for it (its state, vertex buffer and uniforms, as `coax/web/host.js` sizes
its arena). A `BudgetMeter` counts each frame. `budget_take` takes a request
whole or refuses all of it, counts the refusal, marks the frame over budget
and keeps the first refusal (frame, kind, request, used, limit) for the
report; `budget_ok` is what a check asserts.

- A declaration is valid only when its vertex limit is below the realm's
  vertex buffer (`budget_declared`). The realms' quad writers stop at a full
  buffer, so a clipped frame always passes the limit and fails the check.
- Every realm's file declares one (`GARDEN_BUDGET`, `STAIR_BUDGET`, ...) and
  has a check, "... frames fit its declared budget", that draws three frames
  at each of three screens (1440 by 900 at 1x and 2x, a 390 by 844 phone at
  3x). The limits are the native peaks of 120 frames a screen plus a
  quarter (vertices) or a tenth (bytes); book chapter 16 has the table.
- The first run found Hold the Stair's swarm stage drawing 39,318 vertices a
  frame into a 36,000-vertex buffer: 3,318 clipped every frame. Its buffer
  is now 52,000 and its budget 48,000, and with the old buffer its check
  fails.

Checks: the planted overflow (one vertex too many is refused whole and
reported), takes, peaks and ties, measured frames over budget in each kind,
counts below zero, unknown kinds and a sum past the largest `int`.
Mutation: 139 of 139 killed. Steps are measured per realm (Loop, above):
Hold the Stair's, Orbit League's and Dead Gears' budget checks take each
frame's steps, a 3 s stall's included, and `verse/player.coax` checks the
walkers'. Content-hashed asset identity and refusal inside the writers are
still to do.

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
  waiting for a step in its input buffer. A blur, which releases the held
  keys outside any frame, starts a new record the same way.

Checks: `verse/gridwalk.coax` (the keyboard and the touch stick give the
same digests; a tap in a frame with no step jumps on the next; look and zoom
total the same at 30, 60 and 144 FPS; a recorded session replays to the same
digest at every step and a changed frame is found at its step; full records
start new ones that replay), and `coax/web/replay-hosts.test.ts`: a 1,000-frame
Grid session (keys, Shift, Space, arrows, a drag, the wheel, the stick, a
full push, a look finger, a pinch and taps, at 60, 120 and 144 FPS with a
stall) gives the same digest at every one of its 673 steps natively and in
wasm. The session turns slowly while you walk, so its steps take the sine and
cosine of angles where macOS's libm and wasi-libc round differently (sinf
below a quarter of pi and past three quarters, cosf between, about 3 inputs
in 100): with `sin` or `cos` in place of `math_sin` or `math_cos` in the
walker's steering, the check fails at step 243 or 434.
