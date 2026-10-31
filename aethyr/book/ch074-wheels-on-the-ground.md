# 74. Wheels on the ground

> **Status: unimplemented — design draft.** Today
> [`verse/vehicle.coax`](../../verse/vehicle.coax) drives Orbit League's
> cars: one rigid body from [`verse/rigid.coax`](../../verse/rigid.coax),
> four suspension rays, tyres inside the friction circle, sticky wheels,
> jumps and dodges, against a list of planes. It is a game-layer module
> that only Orbit League loads. This chapter plans a vehicle controller on
> Aethyr's rigid bodies and queries. Nothing in `aethyr/` runs yet.

A wheeled vehicle is a rigid body with a few rays and a tyre model. It
doesn't need a full articulated wheel model to feel right in a game. It
does need the same ground queries as the character, the same fixed step,
and the same replay, so a car race replays exactly.

## What this chapter teaches

- How raycast suspension turns a ground query into a spring and damper
  force.
- How a friction circle limits drive, brake and side grip together.
- How sticky wheels and air control work, and where they leave real
  physics.
- When to use this model and when to use Part IV's articulated bodies.

## Outline

1. **The spec as data.** `VehicleSpec` holds the chassis half extents, four
   wheel mounts in the body frame (0 and 1 front, 2 and 3 rear), wheel
   radius, rest travel, stiffness (N/m), damping (N s/m), tyre friction
   `mu`, grip, drive and brake force, top speed, steering lock with
   `steer_fade` at speed, `stick`, `stick_speed` (8 m/s), jump speed and air
   turn rate. The reference car is a 4.2 m by 1.9 m, 1,000 kg roadster
   (`vehicle_roadster`).
2. **Suspension rays.** Each step, each wheel casts down the chassis's own
   −Y. A wheel that reaches a surface pushes along that surface's normal
   with a spring and damper. In Aethyr the cast is
   [chapter 7](ch007-ask-the-world-where-things-are.md)'s raycast, not a
   private list of planes.
3. **Tyres.** Drive or brake acts along the wheel and grip acts against
   side slip, and together they never exceed `mu` times the wheel's load.
   The front wheels steer, less at speed. The handbrake removes most of the
   rear's side grip, so the tail swings out.
4. **Driving on any surface.** The rays follow the chassis, so a wheel can
   meet a wall or a ceiling. While wheels touch, `stick` pulls the car to
   the surface: always on a floor, and on a wall or ceiling only above
   `stick_speed`. This rule is declared game physics, not engineering
   physics.
5. **In the air.** `air` turns the chassis (pitch from throttle, yaw from
   steer, roll with the handbrake). A second jump within 1.25 s
   (`DOUBLE_JUMP_WINDOW`) with the stick past 0.5 (`DODGE_FLICK`) is a
   dodge: 5.5 m/s along the flick and a flip at 9.7 rad/s, with air control
   off for 0.6 s.
6. **Inputs and gravity.** `VehicleInput {throttle, steer, handbrake, jump}`
   comes from [chapter 4](ch004-input-becomes-typed-actions.md)'s typed
   actions. Gravity is an acceleration from the gravity field
   (`gravity_at`), so the same car drives in a spinning drum.
7. **Where it stops.** This controller is for games. A robot's wheels,
   a suspension study or a vehicle that must match measured data uses
   [chapter 19](ch019-joints-in-generalized-coordinates.md)'s joints and
   [chapter 22](ch022-contact-as-an-optimization.md)'s contact instead.

## What exists today

- [verse/vehicle.coax](../../verse/vehicle.coax): the controller, with
  `#test` checks that a roadster settles level, reaches top speed, steers,
  brakes, jumps, dodges, drifts, sticks to a wall when fast and falls off
  when slow, and that the same inputs give the same path bit for bit.
- [verse/contact.coax](../../verse/contact.coax) and
  [verse/rigid.coax](../../verse/rigid.coax): the chassis box against planes,
  and the rigid body.
- [docs/verse/space.md](../../docs/verse/space.md): the space primitives
  table that lists the vehicle beside gravity, thrust and orbits.

## What it builds

- `aethyr/vehicle/wheel.coax`: suspension rays, tyres and the friction
  circle over Aethyr's queries and rigid bodies.
- `aethyr/vehicle/control.coax`: steering, handbrake, stick, air control,
  jumps and dodges as declared game rules.

## Acceptance checks and oracles

The chapter is done when:

- Every `#test` in today's `verse/vehicle.coax` passes against the Aethyr
  modules.
- A car on a heightfield ground and on a box arena takes its wheel contacts
  from the shared query module, with the same answers the character gets.
- A recorded Orbit League match replays to the same digests
  ([chapter 5](ch005-replay-a-session-exactly.md)).
- A wheel's horizontal force never exceeds `mu` times its load, checked
  over a recorded drive.
- Game rules such as `stick` and the dodge are listed in the realm's rule
  module and are absent from any engineering profile.

## Dependencies

Chapters 1 (the fixed step), 4 (actions), 5 (replay), 7 (queries) and 18
(rigid bodies). Chapters 19 and 22 for articulated alternatives. Chapter 63
for Orbit League. From Coax: R12 quantities for forces and speeds, and R20
rule modules for the game-only rules.
