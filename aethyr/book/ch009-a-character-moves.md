# 9. A character moves

> **Status: unimplemented — design draft.** The walking controller in
> `verse/player.coax` (a port of aethyr-cpp's `src/player_core.cpp`) already
> walks, runs, jumps, lands, slides off steep floors and wades at a realm's
> pace; it also owns a third-person camera and its own fixed step. This
> chapter plans the controller as an Aethyr module that reads only typed
> actions and the shared queries. Nothing here runs yet.

A character controller turns intent into motion that feels right: it
accelerates and stops crisply, turns to face where it goes, jumps a fixed
height, lands without bouncing, steps up kerbs, slides off roofs, and never
stutters at a frame rate. Most of that is a handful of well-chosen
constants and an order of operations. Getting that order the same in every
realm is the point of this chapter.

## What this chapter teaches

- Velocity steering toward a wish direction at a fixed acceleration.
- Gravity, jump impulse and peak height.
- Standing, landing, falling and sliding as states.
- Turning to face the direction of travel.
- What a controller reads (actions, queries) and what it writes (state).

## The method

Each fixed step, given `Actions` (chapter 4) and the camera's yaw, the wish
direction `w` is the move axis rotated into world space, zero inside the
dead zone. Speed is `WALK = 3.5 m/s` or `RUN = 7.5 m/s`, times the realm's
`pace` (0.5 wading). Horizontal velocity steps toward the wish at a fixed
rate, scaled by grip:

```jai
Δv     = w · speed − v_xz
v_xz  += Δv / |Δv| · min(|Δv|, ACCEL · grip · h)     // ACCEL = 60 m/s²
jump   : if jump.edge && on_ground: v_y = JUMP       // 7.5 m/s
peak   = JUMP² / (2 · GRAVITY) = 7.5² / 48 ≈ 1.17 m   // GRAVITY = 24 m/s²
```

Then the order is fixed: move across (chapter 8 resolution), query the
ground under the new position (chapter 7), and classify:

- within `RIDE_HEIGHT` above a steep floor and not rising: slide, gravity's
  pull projected along the slope, grip reduced to `SLIDE_GRIP`;
- within `RIDE_HEIGHT` and standing: ease onto the floor;
- within reach and about to cross it this step: land;
- otherwise: fall, `v_y −= GRAVITY · h`.

The character turns toward `atan2(w_x, w_z)` at `TURN_RATE`, along the short
way. The animation state (idle, walking, running, airborne) is derived, not
stored independently, so it can never disagree with the motion.

The controller does not own the camera or the clock. It reads the yaw the
camera reports (chapter 10) and runs inside chapter 1's loop. Interpolated
feet (chapter 2's authoritative position) are what everything draws.

## What exists today

- [verse/player.coax](../../verse/player.coax): `WALK`, `RUN`,
  `RUN_BY_DEFAULT`, `GRAVITY :: 24.0`, `JUMP :: 7.5`, `ACCEL :: 60.0`,
  `TURN_RATE`, `RIDE_HEIGHT`, `STEP_SMOOTH`, `SLIDE_GRIP`; `Player` with
  `pos`, `vel`, `face`, `on_ground`, `act`, `pace`, `grip`, `sliding`;
  `wish`, `step` and `advance_in`; also the orbit camera fields (`theta`,
  `phi`, `dist`, `spring_vel`) and `camera_room` inside the same step.
- `pace` set by realms (`pace_of` in `verse/everglade/river.coax`), `grip`
  for ice in Everglade.
- Realms that keep their own feet: Everglade's `s[0].feet` with `ground_at`,
  Dead Gears' `pl.on_ground` in `apps/openagents-web/deadgears.coax`.
- [verse/gridwalk.coax](../../verse/gridwalk.coax): the Grid's session.
  Its fixed steps take each frame's `Intent` from the session's record
  (chapter 5), sample the input buffer once a step (chapter 4) and hand the
  controller that step's `Actions`: the move axes, run, a jump press, and
  that step's share of look and zoom. Its record replays to the same digest
  at every step, natively and in wasm.
- [verse/terrain.coax](../../verse/terrain.coax) followers
  (`FOLLOW_WALK`, `FOLLOW_ACCEL`, `FOLLOW_TURN`): non-player walkers with
  their own constants.

## What it builds

- `aethyr/walk.coax`: the controller step over `Actions`, `Colliders` and a
  camera yaw; the constants as a declared, per-realm-overridable profile;
  the derived animation state. The camera fields leave `Player` for
  chapter 10.
- Followers move onto the same step with their own profile (speed, turn),
  so NPCs and players share ground, slope and step rules.

## Acceptance checks and oracles

- Jump height: a standing jump peaks at `JUMP² / (2 GRAVITY)` within one
  step's travel; the closed form is the oracle.
- Frame-rate independence: the same action stream at 30, 60, 120 and 144 FPS
  gives identical trajectories (chapter 1).
- Ledges: walking off a ledge, landing from any height and stepping up a
  0.55 m top give the same results in every realm's collider set.
- Acceleration: from rest to `RUN` in `RUN / ACCEL` seconds, rounded up to
  whole steps, on full grip.
- Slopes: a floor steeper than 55 degrees slides the character to its low
  edge; one at 50 degrees holds it.
- Replay: a recorded walk replays bit-identically (chapter 5).

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md),
[4](ch004-input-becomes-typed-actions.md),
[7](ch007-ask-the-world-where-things-are.md) and
[8](ch008-ground-and-collision.md). From Coax: R12 units and R14 bounded
control for the step ([Coax roadmap](../../coax/ROADMAP.md)). Background:
[game-engine systems inventory, section 35](../../docs/research/game-engine-systems.md).
