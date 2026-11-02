# 9. A character moves

> **Status: runnable — native and Wasm fixed-step walking.**
> `aethyr/walk.coax` owns the controller and declared locomotion tuning;
> `aethyr/player.coax` retains its C facade. The Garden and villa's fetching
> dogs consume the same paid steps and drawing alpha. The core, facade,
> replay and actual scene adapters have separate qualification records.
> Complete special realm adapters remain chapter 64's migration.

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

## The declared tuning

<!-- source: aethyr/locomotion.coax profile -->
```jai
LocomotionProfile :: struct {
  walk_speed: f32 = 3.5; run_speed: f32 = 7.5;
  gravity: f32 = 24; jump_speed: f32 = 7.5;
  acceleration: f32 = 60; turn_rate: f32 = 10;
  ride_height: f32 = 0.4; step_smooth: f32 = 10;
  slide_grip: f32 = 0.1; sink_limit: f32 = 0.05;
  run_animation_margin: f32 = 0.25;
}
```

The default `LocomotionProfile` keeps these numbers. A realm supplies its
own profile through `World.locomotion`; it changes no process-wide constant.
Every field is finite, nonnegative and below 1,000; gravity and acceleration
are positive, run speed is at least walk speed, and slide grip is at most one.
Run speed times the fixed period must fit the collision profile's travel
limit. Invalid tuning is refused before a step takes a waiting press. The default profile preserves the existing
controller's results. Walking uses the 1/64 s period; a different positive
clock period or a cap above eight is refused before accepting frame time,
rather than paying one duration and integrating another.

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

An admitted step first takes a waiting jump press. Chapter 4's input buffer makes sure
each press is taken by exactly one step. From the ground, the press sets the
vertical speed to `JUMP`:

<!-- source: aethyr/walk.coax take -->
```jai
// One fixed step. It first takes a waiting jump press: from the ground, it jumps.
step :: (w: World, given: Player, dt: f32) -> Player {
  if !locomotion_profile_ok(w.locomotion) || w.locomotion.run_speed * f32(STEP) > w.collision.max_travel {
    return collision_report(given, CollideResult.{state = collision_state(given), status = CollideStatus.bad_profile});
  }
  p := given;
  t := input_take(given.jumps);
  p.jumps = t.edge;
  if t.pressed && p.on_ground {
    p.vel.y = w.locomotion.jump_speed;
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
5. Move vertically, with the default profile allowing at most 5 cm below the ground.
6. Turn toward the wish, at `TURN_RATE`, the short way round.
7. Set `act` (idle, walking, running or airborne) from this step's motion.
8. After an accepted physical step, `step_once` tracks the camera
   (chapter 10). A refused step leaves the physical state and queued press
   intact and skips camera tracking; the scheduler still pays its time.

<!-- source: aethyr/walk.coax move -->
```jai
  next := p;
  dir := wish(p);
  speed := (ifx p.run then w.locomotion.run_speed else w.locomotion.walk_speed) * clamp(ifx finite(p.pace) then p.pace else 1.0f32, 0, 1);

  // Horizontal velocity steps toward the wish at a fixed rate.
  dx := dir.x * speed - next.vel.x;
  dz := dir.z * speed - next.vel.z;
  diff := sqrt(dx * dx + dz * dz);
  if diff > 0 {
    grip := ifx finite(p.grip) && p.grip > 0 && p.on_ground then lo(p.grip, 1.0f32) else 1.0f32;
    if p.sliding { grip = w.locomotion.slide_grip; }
    applied := lo(diff, w.locomotion.acceleration * grip * dt);
    next.vel.x += dx / diff * applied;
    next.vel.z += dz / diff * applied;
  }
  contact := collide_move(colliders(w), w.collision, collision_state(next), next.vel.x * dt, next.vel.z * dt);
  // A refused step keeps the input Player, including its queued jump. The
  // outer FrameClock consumes the assigned step; only diagnostics change.
  if contact.status != CollideStatus.ok { return collision_report(given, contact); }
  next = collision_report(collision_apply(next, contact.state), contact);

  // Gravity and the floor.
  surface := query_ground_checked(colliders(w), next.pos.x, next.pos.z, next.pos.y + w.collision.step_up, w.collision.radius);
  if surface.status != QueryStatus.ok {
    refused := collision_report(given, CollideResult.{state = collision_state(given), status = CollideStatus.bad_state});
    refused.query_status = surface.status;
    return refused;
  }
  under := StandHit.{y = surface.y, gx = surface.gx, gz = surface.gz};
  ground := under.y;
  above := next.pos.y - ground;
  falls := true;
  next.sliding = false;
  if above <= w.locomotion.ride_height && next.vel.y <= 0 && stand_steep(under) {
    // Too steep to stand on: it holds you as you slide down it, gravity's pull along it speeding
    // you toward its low edge, and over that you fall.
    gl := sqrt(under.gx * under.gx + under.gz * under.gz);
    pull := w.locomotion.gravity * gl / (1 + gl * gl) * dt;
    next.vel.x -= under.gx / gl * pull; next.vel.z -= under.gz / gl * pull;
    next.vel.y = 0;
    next.pos.y = ground;
    next.on_ground = false;
    next.sliding = true;
    falls = false;
  } else if above <= w.locomotion.ride_height && next.vel.y <= 0 {
    if next.on_ground {
      // Standing: ease onto the floor.
      next.vel.y = 0;
      next.pos.y += (ground - next.pos.y) * lo(1, w.locomotion.step_smooth * dt);
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
    next.vel.y -= w.locomotion.gravity * dt;
    next.on_ground = false;
  }
  next.pos.y = hi(next.pos.y + next.vel.y * dt, ground - w.locomotion.sink_limit);

  // Turn toward where you are going.
  if dir.on {
    want := atan2(dir.x, dir.z);
    next.face = wrap_angle(next.face + wrap_angle(want - next.face) * lo(1, w.locomotion.turn_rate * dt));
  }

  if !next.on_ground { next.act = Act.airborne; }
  else if !dir.on { next.act = Act.idle; }
  else if speed > w.locomotion.walk_speed + w.locomotion.run_animation_margin { next.act = Act.running; }   // shown running only at a run's speed
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
#load "../player.coax";
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

## Change a realm's tuning

The second example, [tune-a-walk.coax](../examples/tune-a-walk.coax), declares
a slower run, gentler acceleration and a different jump:

<!-- source: aethyr/examples/tune-a-walk.coax profile -->
```jai
tuned_walk_world :: () -> World {
  world: World;
  world.locomotion.walk_speed = 2;
  world.locomotion.run_speed = 4;
  world.locomotion.acceleration = 16;
  world.locomotion.gravity = 16;
  world.locomotion.jump_speed = 6;
  return world;
}
```

At 16 m/s², each step adds 0.25 m/s, so 4 m/s takes 16 steps. A 6 m/s jump
under 16 m/s² gravity loses 0.25 m/s each step. Its last rising step is 23:
`(6 × 23 − 0.25 × 276) / 64 = 69/64 = 1.078125` m. The following step
has zero vertical speed; the controller lands on step 48.

```shell
coax/bin/coax run aethyr/examples/tune-a-walk.coax
```

```text
declared run: 16 steps, 0.25 s to 4 m/s
declared jump: 1.078125 m after 23 steps; landed after 48
10 ms then 6 ms: 0 then 1 targets, 1 paid tick
step target: z 0.00390625, drawn z 0.00009375
gravity 0: status 1 (bad_profile), jump waiting 1, paid ticks 1
```

The last line deliberately supplies zero gravity. The receipt reports
`CollideStatus.bad_profile` (1), leaves the jump waiting, and pays the assigned
tick without publishing a new physical pose. These are runtime profile
checks; they do not claim Coax's future static bounded-control proof.

## Followers consume the same steps

Counted frame time is for animation; it does not say how many simulation
steps ran. A clock with 10 ms carried into a 6 ms frame owes one 15.625 ms
step. Dividing the new frame's duration by the step would lose it.

`advance_traced` returns the resulting Player and at most eight actual
post-step targets. The trace is temporary caller-owned data, outside Player
and the Grid's persistent digest. A refused physical step still has a target:
the unchanged pose for that paid tick. Drawing consumes the resulting clock's
alpha and never adds another follower accumulator.

<!-- source: aethyr/examples/tune-a-walk.coax trace -->
```jai
main :: () {
  measured := tuned_walk_measure();
  print("declared run: % steps, % s to 4 m/s\n", measured.acceleration_steps, f32(measured.acceleration_steps) * f32(STEP));
  print("declared jump: % m after % steps; landed after %\n", measured.peak, measured.peak_step, measured.landing_step);

  world := tuned_walk_world();
  walker := held(walk_place(0, 0, 0, 0), 0, 1, true);
  first := advance_traced(world, walker, 0.010);
  second := advance_traced(world, first.player, 0.006);
  // A follower consumes the actual targets. Dividing this frame's 6 ms by
  // 1/64 s would miss the step owed by the clock's earlier 10 ms carry.
  print("10 ms then 6 ms: % then % targets, % paid tick\n", first.count, second.count, second.player.clock.ticks);
  print("step target: z %, drawn z %\n", second.targets[0].feet.z, walk_feet(second.player).z);

  invalid := world;
  invalid.locomotion.gravity = 0;
  waiting := walk_input(walk_place(0, 0, 0, 0), 0, 0, 0, 0, false, true, 0, 0);
  refused := advance_traced(invalid, waiting, STEP);
  print("gravity 0: status % (bad_profile), jump waiting %, paid ticks %\n",
        cast(int) cast(u8) refused.player.collision_status, refused.player.jumps.waiting, refused.player.clock.ticks);
}
```

The Garden and villa are the two current consumers of `terrain.Follower`.
Their facades still walk on a floor-relative world. Their scheduled adapters
advance the ball and fetching dog together for each actual target, apply
that realm's terrain origin once, and keep the dog's previous pose. A
controller already stepping on world-height terrain needs no such addition. They draw the same previous/current pair at the player's alpha. A frame
with no tick changes neither physical pose; animation still receives the
clock's counted frame time. Fairy Forest's lantern moths are analytic visual
animation, rather than instances of this follower controller.

## Checks

```shell
coax/bin/coax test aethyr/walk.coax --library
coax/bin/coax test aethyr/player.coax --library
coax/bin/coax test verse/gridwalk.coax --library
```

Checks in `aethyr/player.coax` for this chapter:

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

The coupled scene checks supply the same 2.5 m terrain to native and Wasm
and record 30, 60, 120 and 144 FPS cadences. Physics matches at every observed
paid tick; the actual Garden body records and villa draw getter match the
independent previous/current interpolation. Controls that repeat only the
last target, use variable frame time, or drop the terrain origin all fail.
This qualifies the supplied-height walking/fetch profile. Procedural terrain
generation, raster images and special movement remain separate checks.

The [Walk gate](../tests/walk-qualification.json),
[profile gate](../tests/locomotion-qualification.json),
[follower gate](../tests/follower-qualification.json) and
[Grid adoption](../tests/grid-walk-adoption-qualification.json) record 1,053
root mutants killed with no survivors or waivers. Dependency gates retain
their own scope. The [chapter evidence](../../docs/verification/data/aethyr-ch9-2026-11-01/requirements.json)
records the C facade parity, actual scene checks and complete Wasm links.

## The module and the realm boundary

`aethyr/walk.coax` owns Player, World, physical stepping and interpolation.
The Walk unit declares no C exports of its own. `aethyr/player.coax` retains
the published `verse_player_*` C signatures as thin adapters to `walk_*`.
Coax consumers that qualify names load Walk directly; an alias cannot
re-export a different module's types. Related generated C headers must still
be rebuilt together.

The Input dependency still has the legacy `input_runs` C export. Each
production page reaches one copy. The Dead Gears boundary probe links its
engine and the Player facade together and prefixes that duplicate helper in
a test-only C include wrapper. The Coax arithmetic and facade symbols stay
intact. Consolidating that remaining Input C surface belongs to chapter 64;
Walk's lack of direct exports does not guarantee arbitrary graphs can be
linked as separate C libraries.

The native parity harness compiles the published controller from a pinned
commit separately, then compares every old Player field. It also compiles a
driver against the old header while linking the new facade. This checks the
compatibility claim against an independent frozen implementation.

Chapter [64](ch064-every-realm-runs-on-aethyr.md) still owns every realm's
complete coordinate and movement adapter migration, including Fairy Forest
flight and Everglade's boat, warp and other special movement. Everglade's
`s[0].feet` is a rendering and controller handoff field; Dead Gears already
uses the shared Player. Those fields do not establish separate movement
integrators. This chapter's fixed-step walking and the two current fetching
dogs do not establish whole-realm equivalence for those special paths.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md),
[4](ch004-input-becomes-typed-actions.md),
[7](ch007-ask-the-world-where-things-are.md) and
[8](ch008-ground-and-collision.md). From Coax: R12 units and R14 bounded
control for the step ([Coax roadmap](../../coax/ROADMAP.md)). Background:
[game-engine systems inventory, section 35](../../docs/research/game-engine-systems.md).
