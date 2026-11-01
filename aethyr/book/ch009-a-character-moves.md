# 9. A character moves

> **Status: partly implemented.** The walking controller runs in
> [verse/player.coax](../../verse/player.coax): it steers at a fixed
> acceleration, jumps, stands, lands, falls, slides and turns in a fixed
> order, takes typed actions (chapter 4) and asks chapter 7's queries for the
> ground. The Grid drives it from a recorded session in
> [verse/gridwalk.coax](../../verse/gridwalk.coax). Still to do: the module
> `aethyr/walk.coax`, a per-realm profile of its constants, followers on the
> same step, and the realms that keep their own feet. They land with the realm
> migration ([chapter 64](ch064-every-realm-runs-on-aethyr.md)).

A character controller turns input into motion. It speeds up and stops at a
fixed rate, turns to face where it goes, jumps to a fixed height, lands
without bouncing, steps up onto low tops and slides off steep roofs. Most of
this is a few constants and an order of operations. This chapter shows that
order, and the numbers it gives on the 1/64 s step.

## What this chapter teaches

- Steering the velocity toward a wish direction at a fixed acceleration.
- Gravity, the jump speed and the peak height on a fixed step.
- Standing, landing, falling and sliding, decided in one order each step.
- Turning to face the direction of travel.
- What the controller reads and what it writes.

## The constants

<!-- source: verse/player.coax tuning -->
```jai
WALK :: 3.5;            // m/s
RUN :: 7.5;
// The one setting for how you move (the owner's, 2026-10-29; meant to become
// a player's setting): true, moving runs (RUN) and holding Shift walks
// (WALK); false, moving walks and Shift runs. On a phone the stick decides
// either way: a full push runs, a light one walks (verse/grid.coax RUN_PAST).
RUN_BY_DEFAULT :: INPUT_RUN_BY_DEFAULT;   // aethyr/input.coax holds the rule
GRAVITY :: 24.0;        // m/s^2
JUMP :: 7.5;            // m/s up: a 1.17 m peak
ACCEL :: 60.0;          // m/s^2, speeding up and slowing down alike
TURN_RATE :: 10.0;      // how fast the character turns to face its way
RIDE_HEIGHT :: 0.4;     // higher than this above the floor is falling
STEP_SMOOTH :: 10.0;
```

`SLIDE_GRIP` (0.1) is the share of your push you keep on a slope too steep
to stand on. `STEP_UP`, the steep slope and the floor pad come from
`aethyr/query.coax` (chapter 7).

## The wish

The controls arrive as a frame's `Intent` (chapter 4): the move axes, run, a
jump press and the look. `wish` takes the stick when it is past the dead zone,
and the keys otherwise. It turns the result from the camera's frame into the
world with the camera's turn, `theta`, using `math_sin` and `math_cos`
(chapter 5), so the step has the same bits natively and in the browser.

## One step

A step first takes a waiting jump press. Chapter 4's input buffer makes sure
each press is taken by exactly one step. From the ground, the press sets the
vertical speed to `JUMP`:

<!-- source: verse/player.coax take -->
```jai
// One fixed step. It first takes a waiting jump press: from the ground, it jumps.
step :: (w: World, given: Player, dt: f32) -> Player {
  p := given;
  t := input_take(given.jumps);
  p.jumps = t.edge;
  if t.pressed && p.on_ground {
    p.vel.y = JUMP;
    p.on_ground = false;
  }
```

The rest of the step runs in this order:

1. Steer the horizontal velocity toward `wish × speed` by at most
   `ACCEL × grip × dt`. `speed` is `WALK` or `RUN` times the realm's `pace`.
   `grip` is below 1 on a realm's ice, and `SLIDE_GRIP` while sliding.
2. Move across and resolve the move against the colliders (chapter 8).
3. Ask for the ground under the new position (chapter 7's `query_stand`).
4. Decide what you are doing. Within `RIDE_HEIGHT` of a steep surface and not
   rising, you slide: gravity's pull along the slope is added to your speed.
   Within `RIDE_HEIGHT` and standing, you ease onto the floor at
   `STEP_SMOOTH`. Falling and reaching the floor this step, you land on it.
   Otherwise you fall, and gravity takes `GRAVITY × dt` off your vertical
   speed.
5. Move vertically, never more than 5 cm below the ground.
6. Turn toward the wish, at `TURN_RATE`, the short way round.
7. Track the camera (chapter 10).
8. Set `act` (idle, walking, running or airborne) from this step's motion.
   It is never set anywhere else, so it can't disagree with the motion.

<!-- source: verse/player.coax move -->
```jai
  next := p;
  dir := wish(p);
  speed := (ifx p.run then f32(RUN) else f32(WALK)) * clamp(ifx finite(p.pace) then p.pace else 1.0f32, 0, 1);

  // Horizontal velocity steps toward the wish at a fixed rate.
  dx := dir.x * speed - next.vel.x;
  dz := dir.z * speed - next.vel.z;
  diff := sqrt(dx * dx + dz * dz);
  if diff > 0 {
    grip := ifx finite(p.grip) && p.grip > 0 && p.grip < 1 && p.on_ground then p.grip else 1.0f32;
    if p.sliding { grip = f32(SLIDE_GRIP); }
    applied := lo(diff, f32(ACCEL) * grip * dt);
    next.vel.x += dx / diff * applied;
    next.vel.z += dz / diff * applied;
  }
  before := next;
  next = move_across(w, next, next.vel.x * dt, next.vel.z * dt);
  next = off_floors(w, before, next);

  // Gravity and the floor.
  under := floor_under(w, next.pos.x, next.pos.z, next.pos.y);
  ground := under.y;
  above := next.pos.y - ground;
  falls := true;
  next.sliding = false;
  if above <= f32(RIDE_HEIGHT) && next.vel.y <= 0 && stand_steep(under) {
    // Too steep to stand on: it holds you as you slide down it, gravity's pull along it speeding
    // you toward its low edge, and over that you fall.
    gl := sqrt(under.gx * under.gx + under.gz * under.gz);
    pull := f32(GRAVITY) * gl / (1 + gl * gl) * dt;
    next.vel.x -= under.gx / gl * pull; next.vel.z -= under.gz / gl * pull;
    next.vel.y = 0;
    next.pos.y = ground;
    next.on_ground = false;
    next.sliding = true;
    falls = false;
  } else if above <= f32(RIDE_HEIGHT) && next.vel.y <= 0 {
    if next.on_ground {
      // Standing: ease onto the floor.
      next.vel.y = 0;
      next.pos.y += (ground - next.pos.y) * lo(1, f32(STEP_SMOOTH) * dt);
      falls = false;
    } else if above + next.vel.y * dt <= 0 {
      // Lands this step.
      next.vel.y = 0;
      next.pos.y = ground;
      next.on_ground = true;
      falls = false;
    }
  }
  if falls {
    next.vel.y -= f32(GRAVITY) * dt;
    next.on_ground = false;
  }
  next.pos.y = hi(next.pos.y + next.vel.y * dt, ground - 0.05);

  // Turn toward where you are going.
  if dir.on {
    want := atan2(dir.x, dir.z);
    next.face = wrap_angle(next.face + wrap_angle(want - next.face) * lo(1, f32(TURN_RATE) * dt));
  }

  // Follow on the spring, and pull in at once where something is in the
  // way; back out gently.
  cam := follow_track(follow_of(next), colliders(w), w.room, wp(look_at(next)), dt);
  next.target = vp(cam.target); next.spring_vel = vp(cam.vel); next.dist = cam.dist;

  if !next.on_ground { next.act = Act.airborne; }
  else if !dir.on { next.act = Act.idle; }
  else if speed > f32(WALK) + 0.25 { next.act = Act.running; }   // shown running only at a run's speed
  else { next.act = Act.walking; }
  return next;
}
```

## The numbers on a fixed step

Work them out by hand before you run the program.

1. **Speeding up.** One step adds `ACCEL × dt = 60 / 64 = 0.9375` m/s. `RUN`
   is 7.5 m/s, so you reach it in `7.5 / 0.9375 = 8` steps, which is
   `RUN / ACCEL = 0.125` s.
2. **The jump.** The step lowers the speed first and then moves, so after
   step `k` of a jump you have risen `Σ (7.5 − 0.375 i) / 64` m for `i` from 1
   to `k`. The speed reaches 0 at step 20, so you stop rising after step 19,
   at `(7.5 × 19 − 0.375 × 190) / 64 = 71.25 / 64 = 1.11328125` m. The
   closed form `JUMP² / (2 GRAVITY)` gives 1.171875 m. The difference is
   `JUMP × dt / 2 = 0.0586` m, the half step this order of operations loses.
3. **The landing.** The fall mirrors the rise, so you land after 40 steps,
   exactly on the ground.
4. **Slopes.** `tan 50° = 1.19` is below `STAND_STEEP` (`tan 55° = 1.43`), so
   a 50 degree slope holds you. `tan 60° = 1.73` is above it, so a 60 degree
   slope slides you to its low edge, and you drop onto the ground beyond it.

The example [aethyr/examples/run-and-jump.coax](../examples/run-and-jump.coax)
measures each of these:

<!-- source: aethyr/examples/run-and-jump.coax example -->
```jai
#load "../../verse/player.coax";
#import "Basic";

// The walking controller on the 1/64 s step: how many steps it takes to reach RUN, how high
// a standing jump goes, where it lands, and what a 50 and a 60 degree slope do to you.
ramp :: (degrees: f32) -> Player {
  f: [STAND_FLOATS] f32;
  rise := 4 * sin(degrees * f32(PI) / 180) / cos(degrees * f32(PI) / 180);
  stand_put_quad(f[..], 0, -2, 0, 4, 0, 0, 4, 0, 0, rise);   // 4 m by 4 m, rising toward +Z
  w: World; w.floors = f[..];
  p := verse_player_place(0, rise / 2 + 0.1, 2, 0);              // over its middle, 0.1 m up
  for k: 0..<128 { p = step(w, p, STEP); }
  return p;
}
main :: () {
  w: World;
  p := held(verse_player_place(0, 0, 0, 0), 0, 1, true);
  n := 0;
  while flat_speed(p) < f32(RUN) { p = step(w, p, STEP); n += 1; }
  print("from rest to RUN: % steps, % s\n", n, f32(n) * f32(STEP));

  q := verse_player_input(verse_player_place(0, 0, 0, 0), 0, 0, 0, 0, false, true, 0, 0);
  peak := 0.0f32; up := 0; air := 0;
  while true {
    q = step(w, q, STEP); air += 1;
    if q.pos.y > peak { peak = q.pos.y; up = air; }
    if q.on_ground { break; }
  }
  print("standing jump: peak % m after % steps; JUMP^2 / (2 GRAVITY) = % m\n", peak, up, f32(JUMP * JUMP / (2 * GRAVITY)));
  print("landed after % steps, feet at %, falling %\n", air, q.pos.y, q.vel.y);

  for d: [2] f32.[50, 60] {
    r := ramp(d);
    print("% degrees: after 2 s at (%, %), sliding %, on the ground %\n", d, r.pos.z, r.pos.y, r.sliding, r.on_ground);
  }
}
```

```shell
coax/bin/coax run aethyr/examples/run-and-jump.coax
```

```text
from rest to RUN: 8 steps, 0.125 s
standing jump: peak 1.1132812 m after 19 steps; JUMP^2 / (2 GRAVITY) = 1.171875 m
landed after 40 steps, feet at 0.0, falling 0.0
50.0 degrees: after 2 s at (2.0, 2.383507), sliding false, on the ground true
60.0 degrees: after 2 s at (-0.14296108, 0.0), sliding false, on the ground true
```

Every number matches the hand calculation. On the 50 degree slope you stay at
its middle, 2.38 m up. On the 60 degree slope you end 0.14 m past its low
edge, on the ground.

## Checks

```shell
coax/bin/coax test verse/player.coax --library
coax/bin/coax test verse/gridwalk.coax --library
```

Checks in `verse/player.coax` for this chapter:

- "jump acts on the press, not the hold, and only from the ground";
- "placed in the air, you fall to the floor", "standing eases onto a floor
  within RIDE_HEIGHT", "walking off a ledge falls" and "a fall that reaches
  the floor exactly lands";
- "on ice (a grip under one) the same wish takes you up to speed slower, and
  you slide on" and "a pace under one slows you by its share, and the land
  speeds stay as they are";
- "the character turns to where it moves, at TURN_RATE" and "atan2 behind is
  +PI, not -PI";
- "a slope too steep to stand on slides you off its low edge", and the
  roof checks for landing on a slope and walking along one;
- "jumping onto, landing on and walking off a ledge is the same in every
  realm" (chapter 7).

Frame-rate independence is chapter 1's: the controller runs only on the fixed
step, so 30, 60, 120 and 144 FPS give the same states. Replay is chapter 5's:
the Grid's recorded session replays to the same digest at every step,
natively and in wasm.

## Still to do

The realm migration ([chapter 64](ch064-every-realm-runs-on-aethyr.md)) adds:

- `aethyr/walk.coax`: the step above, moved from `verse/player.coax`, with
  its constants as a declared profile a realm can override.
- Followers on the same step. Today `verse/terrain.coax`'s followers use
  their own constants (`FOLLOW_WALK`, `FOLLOW_ACCEL`, `FOLLOW_TURN`) and
  their own ground rules.
- The realms that keep their own feet: Everglade's `s[0].feet` with
  `ground_at`, and Dead Gears' `pl.on_ground`.

The camera's fields and its spring leave the step in chapter 10.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md),
[4](ch004-input-becomes-typed-actions.md),
[7](ch007-ask-the-world-where-things-are.md) and
[8](ch008-ground-and-collision.md). From Coax: R12 units and R14 bounded
control for the step ([Coax roadmap](../../coax/ROADMAP.md)). Background:
[game-engine systems inventory, section 35](../../docs/research/game-engine-systems.md).
