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
