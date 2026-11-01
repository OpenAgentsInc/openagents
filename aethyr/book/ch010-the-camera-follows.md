# 10. The camera follows

> **Status: unimplemented — design draft.** Several cameras follow a
> character today: the orbit inside `verse/player.coax`'s fixed step (a
> spring on the target, `camera_room` pulling it in), the free `Orbit` in
> `verse/camera.coax` advanced per frame with `verse_orbit_follow`, Ice Cream
> Man's swept-sphere boom in `verse/icecream/camera.coax`, and `SpaceCam`'s
> floating origin in `verse/spacecam.coax`. This chapter plans one follow
> that reads the interpolated player state and is smoothed in one place.
> Engine-core task: `ec-camera` (#12609). The first part has landed:
> `aethyr/follow.coax` holds the walker's follow camera (look, zoom, the
> spring, room from the colliders, the eye between steps), and
> `verse/player.coax` moves its camera only through it. The chapter is
> filled in with that module's listings next; the other cameras here are
> still plans.

Camera judder is almost always a timing mismatch. If the camera chases the
last physics state, it moves in steps of 1/64 s while the screen refreshes
at 60 or 120 Hz, and the character appears to vibrate against the
background. If it smooths in the physics step and again per frame, it lags
twice and differently at each frame rate. If it reaches into physics to
resolve its own collisions, it can change what happens. Aethyr's rule is
narrow: every camera reads the interpolated state (chapter 1, chapter 2),
smooths in exactly one place with frame-rate-independent easing, asks the
query module for room (chapter 7), and never steps physics.

## What this chapter teaches

- Follow, orbit, first-person and dive cameras as one `Shot` producer.
- Exponential easing that does not depend on frame rate.
- Cuts versus follows: when to jump instead of ease.
- Pulling in for scenery with a swept sphere, and backing out gently.
- Handing one camera over to another by blending shots.

## The method

A camera is a function of the drawn player state and its own small state,
evaluated once per frame after the loop has stepped:

```jai
target  = feet_drawn + (0, pivot_height, 0)        // interpolated, chapter 2
pivot   = |target − pivot| > cut ? target
                                 : mix(pivot, target, 1 − exp(−k · Δ))
eye_ray = pivot + shoulder + spherical(yaw, pitch) · zoom
room    = sweep(sphere(r_clear), pivot → eye_ray)   // chapter 7, read-only
shown   = room < shown ? room                       // pull in at once
                       : mix(shown, room, 1 − exp(−k_out · Δ))
shot    = Shot { eye = pivot + dir · shown, at = pivot }
```

`1 − exp(−k Δ)` is the share of the remaining distance closed in time `Δ`;
two frames of `Δ/2` close exactly as much as one of `Δ`, so the follow is
the same at any frame rate. Smoothing happens here and nowhere else: the
player's state carries no camera fields. A move larger than `cut` metres in
one frame (a respawn, a portal) is a cut. Look input (chapter 4's `look`)
changes yaw and pitch directly; recentring behind the walker starts only
after a declared idle time. First person is the zoom's inner limit. Dive
and overview cameras are other producers of the same `Shot`, and a blend
between two shots eases eye and target separately, the target leading.

## What exists today

- [verse/player.coax](../../verse/player.coax): `theta`, `phi`, `dist`,
  `target`, `spring_vel`, `spring`, `camera_room`, `CAM_LERP` "per 1/60 s",
  all updated inside `step`, then interpolated by `verse_player_at` and
  `verse_player_eye`.
- [verse/camera.coax](../../verse/camera.coax): `Shot`, the overview
  (`verse_camera_overview`), Start's dolly (`verse_camera_dolly`, `ease`,
  `bezier`), and `Orbit` with `verse_orbit_follow` (pivot easing by
  `settle`, a `FREE_JUMP` cut, recentring after `FREE_RECENTRE_AFTER`),
  `verse_orbit_room`, `verse_orbit_limit`, `verse_orbit_shot`, first person
  past `FREE_INSIDE`.
- [verse/space.coax](../../verse/space.coax): `settle(rate, dt) = 1 − exp(−rate · dt)`.
- [verse/icecream/camera.coax](../../verse/icecream/camera.coax): a boom
  pulled in by a swept sphere against the house, Shift Lock, first/third
  person on V.
- [verse/spacecam.coax](../../verse/spacecam.coax): f64 floating origin, zoom
  over twelve decades eased in log space.

## What it builds

- `aethyr/follow.coax`: the follow, orbit, first-person and dive cameras as
  `Shot` producers over the interpolated player state, one easing helper,
  cuts, room from chapter 7's sweep, and shot blending. The overview, dolly
  and `SpaceCam` become producers of the same type.
- The player's camera fields move here; `verse/player.coax`'s step no longer
  touches the camera.

## Acceptance checks and oracles

- No frame-to-frame camera jump above a bound during walk, run and jump at
  60 FPS: a scripted run records the eye per frame and checks the largest
  change against the walker's speed times the frame time plus the easing
  bound.
- The Grid's jump is smooth: in a 60 FPS capture the eye height over the
  jump has no reversal on the rising arc and no repeated value while moving.
- Frame-rate independence: the eye after one second of the same motion at
  30, 60, 120 and 144 FPS agrees within a stated tolerance; the closed form
  of exponential easing is the oracle.
- Read-only: a check that physics state digests (chapter 5) are identical
  with the camera evaluated every frame, never, or twice per frame.
- Room: the eye never sits inside a collider; pulled in at once, backed out
  no faster than the declared rate.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md),
[2](ch002-spaces-and-units.md), [4](ch004-input-becomes-typed-actions.md),
[7](ch007-ask-the-world-where-things-are.md) and
[9](ch009-a-character-moves.md). From Coax: R12 frames and R13 clocks
([Coax roadmap](../../coax/ROADMAP.md)). Background:
[game-engine systems inventory, section 36](../../docs/research/game-engine-systems.md).
