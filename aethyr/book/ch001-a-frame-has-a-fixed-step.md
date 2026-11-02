# 1. A frame has a fixed step

> Engine-core task: `ec-loop` (#12609), done in `3102756f8b`. The module is
> [aethyr/loop.coax](../loop.coax). The walking controller in
> [verse/player.coax](../../verse/player.coax), which every walking realm
> uses, and Orbit League step on it.

A display shows frames at its own rate: 60 frames a second on most phones,
120 or 144 on fast screens. A browser tab can also pause for 300 ms or more
when it wakes up. If a simulation advances by each frame's time, a jump
reaches a different height at each frame rate.

Aethyr keeps two clocks apart:

- The simulation advances only in fixed steps.
- Drawing shows a point between the last two steps.

This chapter shows the clock that does both.

## What this chapter teaches

- Why a fixed step makes physics independent of the frame rate.
- How to keep time in whole nanoseconds, so that a step count doesn't depend
  on float rounding.
- How to clamp a long frame, limit the steps in one frame, and drop the
  extra time.
- How to draw between the last two steps.
- How to check that a jump moves smoothly at 60 FPS.

## The method

Each frame, the host reports the time `Δ` since the last frame. The clock
handles it in these steps:

1. Round `Δ` to whole nanoseconds. Refuse a time that is negative, infinite,
   or not a number.
2. Clamp it to `Δ_max`.
3. Add it to the time carried from earlier frames, `a`.
4. Count the whole steps `h` in `a`. If there are more than `N_max`, drop the
   extra steps' time and run `N_max`.
5. After the steps run, compute `α = a / h`, the part of a step left over.
   Drawing mixes the last two steps by `α`.

```jai
Δ'    = clamp(round_ns(Δ), 0, Δ_max)
a     = a + Δ'
n     = ⌊a / h⌋
if n > N_max { a = a − (n − N_max) h; n = N_max }
a     = a − n h                         // after the n steps run: a in [0, h)
α     = a / h                           // in [0, 1)
draw  = lerp(state_prev, state_now, α)
```

Before each step, the realm copies the fields that it draws into `prev_`
fields. After the last step, it draws `lerp(prev, now, α)`. Angles use the
shorter direction: `wrap_angle(now − prev)`.

The clock keeps time as an integer number of nanoseconds. The walking step
is 1/64 s, exactly 15,625,000 ns, so 64 steps are exactly one second. Orbit
League's step is 1/120 s, which rounds to 8,333,333 ns. Both are
deterministic: the same frame times give the same step counts, because the
clock never compares a float total with a step.

The defaults are:

- a step of 1/64 s (`LOOP_STEP_NS`);
- a longest frame of 0.25 s (`LOOP_MAX_FRAME_NS`);
- at most eight steps in one frame (`LOOP_MAX_STEPS`).

## The clock

The clock is a value. A realm keeps one in its state and replaces it each
frame. The clock also counts the frames it refused, the frames it clamped,
the frames with too many steps, and the time it dropped. A realm and its
checks can read these counts.

<!-- source: aethyr/loop.coax clock -->
```jai
LOOP_SECOND :: 1000000000;        // nanoseconds
LOOP_STEP_NS :: 15625000;         // 1/64 s, exact: the walking realms' step
LOOP_MAX_FRAME_NS :: 250000000;   // the longest frame that counts
LOOP_MAX_STEPS :: 8;              // steps one frame may run
LOOP_NEVER :: 1000000.0;          // seconds: a frame time this long (or longer) is not one

FrameClock :: struct {
  step_ns: int = LOOP_STEP_NS;    // one fixed step; a clock with none takes LOOP_STEP_NS on its first frame
  max_frame_ns: int = LOOP_MAX_FRAME_NS;
  max_steps: int = LOOP_MAX_STEPS;
  carry_ns: int;     // accepted and not yet stepped: in [0, step_ns) between frames
  due: int;          // steps this frame must run (loop_due), paid by loop_ran
  ticks: int;        // steps run since the clock started: simulation time is ticks * step_ns
  frames: int;       // frames accepted
  refused: int;      // frames whose time was not a time (negative, infinite, not a number)
  overlong: int;       // frames longer than max_frame_ns, counted as that long
  stalls: int;       // frames owed more than max_steps
  dropped_ns: int;   // backlog thrown away on those
  counted_ns: int;   // the time the last frame counted for: refused, clamped and dropped as above
  ran: int;          // steps the last frame ran (loop_ran)
}
```

`loop_accept_ns` does all of the method except running the steps. If the
step, clamp, or step limit is zero or negative, the clock uses the declared
default. A clock read from memory that was never constructed
([chapter 3](ch003-state-starts-from-its-defaults.md)) still runs.

<!-- source: aethyr/loop.coax accept -->
```jai
loop_accept_ns :: (c: FrameClock, frame_ns: int) -> FrameClock {
  next := c;
  // A clock read from zeroed memory takes the defaults it was declared with.
  if c.step_ns <= 0 { next.step_ns = LOOP_STEP_NS; }
  if c.max_frame_ns <= 0 { next.max_frame_ns = LOOP_MAX_FRAME_NS; }
  if c.max_steps <= 0 { next.max_steps = LOOP_MAX_STEPS; }
  next.frames += 1;
  next.ran = 0;
  f := frame_ns;
  if f < 0 { next.refused += 1; f = 0; }
  if f > next.max_frame_ns { next.overlong += 1; f = next.max_frame_ns; }
  next.carry_ns += f;
  next.due = next.carry_ns / next.step_ns;
  if next.due > next.max_steps {
    lost_ns := (next.due - next.max_steps) * next.step_ns;
    next.stalls += 1;
    next.dropped_ns += lost_ns;
    next.carry_ns -= lost_ns;
    next.due = next.max_steps;
    f -= lost_ns;
  }
  next.counted_ns = f;
  return next;
}
```

The realm runs the steps that are due, then calls `loop_ran`. `loop_alpha`
returns the value that drawing mixes by.

<!-- source: aethyr/loop.coax ran -->
```jai
// How many steps to run now.
loop_due :: (c: FrameClock) -> int { return c.due; }

// The steps due have run: they are paid, and simulation time moves on by them.
loop_ran :: (c: FrameClock) -> FrameClock {
  next := c;
  next.carry_ns -= c.due * c.step_ns;
  next.ticks += c.due;
  next.ran = c.due;
  next.due = 0;
  return next;
}

// One frame in one call: accept its time, and the steps it runs.
FrameTick :: struct { clock: FrameClock; steps: int; }
loop_frame :: (c: FrameClock, seconds: f64) -> FrameTick {
  a := loop_accept(c, seconds);
  return FrameTick.{clock = loop_ran(a), steps = a.due};
}

// How far this frame lies from the last step to the next, in [0, 1), once the steps due
// have run: what drawing mixes the last two by.
loop_alpha :: (c: FrameClock) -> f32 {
  return cast(f32) (cast(f64) (c.carry_ns - c.due * c.step_ns) / cast(f64) c.step_ns);
}
```

## One owner of time

The walking controller uses the clock as follows:

1. `advance_in` passes the frame's time to `loop_accept`.
2. It runs the number of steps that `loop_due` returns, and saves the
   position, target, facing, and camera distance before each step. Each step
   is the player's step and then the camera's (`step_once`, chapter 10).
3. It calls `loop_ran`.

No other code in the controller reads the frame's time.

<!-- source: aethyr/walk.coax advance -->
```jai
// Advances by a frame's time in fixed steps, on the player's clock (aethyr/loop.coax):
// the frame's time is clamped, at most LOOP_MAX_STEPS run, and a longer backlog is
// dropped, so a stall never makes a frame slower.
advance_in :: (w: World, p: Player, frame: f32) -> Player {
  return advance_traced(w, p, frame).player;
}

AdvanceTrace :: struct { player: Player; count: int; targets: [LOOP_MAX_STEPS] StepTarget; }
WalkSchedule :: struct { ok: bool; clock: FrameClock; count: int; }
// A new walking frame starts from a paid fixed clock. Zeroed clock defaults
// normalize through loop_accept, but a different positive period is refused.
walk_schedule :: (clock: FrameClock, frame: f32) -> WalkSchedule {
  if (clock.step_ns > 0 && clock.step_ns != LOOP_STEP_NS) || clock.max_steps > LOOP_MAX_STEPS || clock.carry_ns < 0 || clock.carry_ns >= LOOP_STEP_NS || clock.due != 0 {
    return WalkSchedule.{};
  }
  accepted := loop_accept(clock, f64(frame));
  count := loop_due(accepted);
  // Canonical carry, the fixed positive period and the admitted step cap
  // make loop_accept's count lie in0..8, including normalized clock defaults.
  return WalkSchedule.{ok = true, clock = accepted, count = count};
}
walk_target :: (p: Player) -> StepTarget {
  return StepTarget.{feet = wp(p.pos), velocity = StepVelocity.{x = p.vel.x, y = p.vel.y, z = p.vel.z},
    face = p.face, act = cast(u8) p.act, collision_status = cast(u8) p.collision_status};
}
advance_traced :: (w: World, p: Player, frame: f32) -> AdvanceTrace {
  out := AdvanceTrace.{player = p};
  schedule := walk_schedule(p.clock, frame);
  if !schedule.ok {
    out.player = collision_report(p, CollideResult.{state = collision_state(p), status = CollideStatus.bad_profile});
    return out;
  }
  next := p; next.clock = schedule.clock; out.count = schedule.count;
  for i: 0..<schedule.count {
    next = history_step(w, next);
    out.targets[i] = walk_target(next);
  }
  next.clock = loop_ran(next.clock); out.player = next;
  return out;
}
history_step :: (w: World, p: Player) -> Player {
  next := p; next.prev_pos = p.pos; next.prev_target = p.target;
  next.prev_face = p.face; next.prev_dist = p.dist;
  return step_once(w, next, STEP);
}
// Runs n fixed steps (what a clock says are due), keeping where the last began, so the
// frame interpolates between the last two.
steps_in :: (w: World, p: Player, n: int) -> Player {
  next := p;
  for i: 0..<n {
    next = history_step(w, next);
  }
  return next;
}
```

Drawing reads only the last two steps and `α`. This applies to the feet, the
facing, and the camera, whose mix is `aethyr/follow.coax`'s `follow_shot`
(chapter 10).

<!-- source: aethyr/walk.coax drawn -->
```jai
// The feet and facing to draw this frame.
walk_feet :: (p: Player) -> Vec3 { return mix3(p.prev_pos, p.pos, alpha(p)); }
walk_face :: (p: Player) -> f32 {
  return wrap_angle(p.prev_face + wrap_angle(p.face - p.prev_face) * alpha(p));
}
// The camera this frame, Aethyr's third-person shot: what it looks at, and where it is. A wall
// pulls the camera in at once, so it is never shown further out than the last step.
walk_shot :: (p: Player) -> CameraShot { return follow_shot(follow_of(p), wp(p.prev_target), p.prev_dist, alpha(p)); }
walk_at :: (p: Player) -> Vec3 { return vp(walk_shot(p).at); }
walk_eye :: (p: Player) -> Vec3 { return vp(walk_shot(p).eye); }
```

## Steps are the same at any frame rate

This check feeds the clock five seconds of frames at 30, 60, 120, and 144
FPS. After every frame, it requires the following:

- The clock has run `⌊time / h⌋` steps.
- The drawn time, `(ticks + α) h`, equals the time fed in.

<!-- source: aethyr/loop.coax rates -->
```jai
// The contract: a run's steps are the same whatever the frame rate. Fed five seconds in
// frames of 30, 60, 120 or 144 a second, the clock has run floor(time / step) steps after
// every frame, never more, and the time drawn, (ticks + alpha) * step, is the time fed.
#forall "the step count follows the time fed at any frame rate" r: 0..<4 {
  fps := ifx r == 0 then 30 else ifx r == 1 then 60 else ifx r == 2 then 120 else 144;
  c: FrameClock;
  fed := 0;
  for f: 0..<fps * 5 {
    fed += loop_ns(1.0 / cast(f64) fps);
    k := loop_frame(c, 1.0 / cast(f64) fps);
    c = k.clock;
    #assert c.ticks == fed / LOOP_STEP_NS && c.dropped_ns == 0;
    #assert c.ticks * LOOP_STEP_NS + c.carry_ns == fed;
    shown := (cast(f64) c.ticks + cast(f64) loop_alpha(c)) * loop_step(c);
    #assert abs(shown - cast(f64) fed / f64(LOOP_SECOND)) < 0.000001;
  }
  #assert c.frames == fps * 5 && c.ticks >= 319 && c.ticks <= 320;
}
```

The example [aethyr/examples/frame-rates.coax](../examples/frame-rates.coax)
feeds the same five seconds, then a three-second pause, and prints what the
clock did:

<!-- source: aethyr/examples/frame-rates.coax example -->
```jai
#load "../loop.coax";
#import "Basic";

// Five seconds fed at four frame rates, then a three-second stall: how many fixed steps
// each runs, the most one frame runs, and the time drawn against the time fed.
main :: () {
  rates := [4] int.[30, 60, 120, 144];
  for r: 0..<4 {
    fps := rates[r];
    c: FrameClock;
    most := 0;
    for f: 0..<fps * 5 {
      k := loop_frame(c, 1.0 / cast(f64) fps);
      c = k.clock;
      if k.steps > most { most = k.steps; }
    }
    drawn := (cast(f64) c.ticks + cast(f64) loop_alpha(c)) * loop_step(c);
    print("% FPS: % frames, % steps, at most % a frame, drawn at % s\n", fps, c.frames, c.ticks, most, drawn);
  }
  c: FrameClock;
  k := loop_frame(c, 3.0);
  print("a 3 s stall: % steps, % ns dropped, overlong %, stalls %\n", k.steps, k.clock.dropped_ns, k.clock.overlong, k.clock.stalls);
  n := loop_frame(k.clock, 1.0 / 60.0);
  print("the next frame: % step\n", n.steps);
}
```

```shell
coax/bin/coax run aethyr/examples/frame-rates.coax
```

```text
30 FPS: 150 frames, 319 steps, at most 3 a frame, drawn at 4.999999949708581 s
60 FPS: 300 frames, 320 steps, at most 2 a frame, drawn at 5.000000100000001 s
120 FPS: 600 frames, 319 steps, at most 1 a frame, drawn at 4.9999997997656465 s
144 FPS: 720 frames, 319 steps, at most 1 a frame, drawn at 4.999999679625034 s
a 3 s stall: 8 steps, 125000000 ns dropped, overlong 1, stalls 1
the next frame: 1 step
```

At 60 FPS, one frame rounds up to 16,666,667 ns. The 300 frames add up to
5.0000001 s, so the clock reaches step 320. The other rates round down and
stop at step 319. The pause counts as 0.25 s: the clock runs eight steps and
drops the time of the other eight. The next frame runs one step, like any
other frame.

The controller has a stronger check, "a run and jump has the same states at
30, 60, 120 and 144 FPS" in `verse/player.coax`. It runs a jump at each rate
and compares every step, bit for bit, with the same jump run one step at a
time. It also requires the drawn feet to lie between the last two steps.

## A jump moves smoothly at 60 FPS

At 60 FPS, the 64 Hz steps don't fall one to a frame. Most frames run one
step, and some run two. If you draw the last step, the jump's height jumps
forward in each frame that runs two steps. If you draw between the last two
steps, each frame's movement changes by about as much as gravity changes it.

The check measures, through the jump, how much each frame's change in height
differs from the previous frame's. It requires that difference to stay under
a bound:

<!-- source: aethyr/walk.coax stutter -->
```jai
// No stutter at 60 FPS: the 64 Hz steps do not land one to a frame (some frames run two),
// yet what is drawn moves smoothly. Through a jump's flight the feet drawn each frame change
// speed by no more than gravity does in a frame plus the interpolation's error (measured
// 0.008 m), and the camera by no more than its spring's own pull adds (0.014 m); drawing the
// last step instead lurches by 0.077 m (feet) and 0.062 m (camera) every few frames.
#test "a jump drawn at 60 FPS has no stutter" {
  w: World;
  p := walk_input(walk_place(0, 0, 0, 0), 0, 1, 0, 0, true, true, 0, 0);
  frame := cast(f32) (1.0 / 60.0);
  bound := f32(GRAVITY) * frame * frame + 0.004;   // 0.0107 m
  eye_bound := 0.016f32;
  was_y := 0.0f32; was_dy := 0.0f32; was_eye := 0.0f32; was_deye := 0.0f32;
  flying := 0;
  for k: 0..<90 {
    p = advance_in(w, p, frame);
    y := walk_feet(p).y;
    eye := walk_eye(p).y;
    dy := y - was_y; deye := eye - was_eye;
    if k >= 2 && !p.on_ground && p.prev_pos.y > 0 && p.clock.ticks > 2 {
      #assert abs(dy - was_dy) <= bound;
      #assert abs(deye - was_deye) <= eye_bound;
      flying += 1;
    }
    was_y = y; was_dy = dy; was_eye = eye; was_deye = deye;
  }
  #assert flying > 30;   // most of a 0.6 s flight was checked
  // Drawing the last step instead (no interpolation) would fail the same bound.
  q := walk_input(walk_place(0, 0, 0, 0), 0, 1, 0, 0, true, true, 0, 0);
  worst := 0.0f32; worst_eye := 0.0f32; was_y = 0; was_dy = 0; was_eye = 0; was_deye = 0;
  for k: 0..<40 {
    q = advance_in(w, q, frame);
    dy := q.pos.y - was_y;
    eye := q.target.y + orbit_offset(q).y * q.dist;
    deye := eye - was_eye;
    if k >= 2 && !q.on_ground { worst = hi(worst, abs(dy - was_dy)); worst_eye = hi(worst_eye, abs(deye - was_deye)); }
    was_y = q.pos.y; was_dy = dy; was_eye = eye; was_deye = deye;
  }
  #assert worst > 3 * bound && worst_eye > 3 * eye_bound;
}
```

The example [aethyr/examples/jump-at-60.coax](../examples/jump-at-60.coax)
prints the largest difference for the same jump:

```shell
coax/bin/coax run aethyr/examples/jump-at-60.coax
```

```text
gravity alone changes a frame's move by 6.7 mm
feet: interpolated 7.0 mm, last step 76.2 mm
camera: interpolated 13.1 mm, last step 61.3 mm
```

| Drawn at 60 FPS | Between the last two steps | At the last step | Bound in the check |
| --- | ---: | ---: | ---: |
| Feet | 7.0 mm | 76.2 mm | 10.7 mm |
| Camera | 13.1 mm | 61.3 mm | 16 mm |

Gravity alone accounts for 6.7 mm (`g Δ²`, where `g` is 24 m/s² and `Δ` is
1/60 s). The camera follows the player on a spring, which adds to its value.
The check also draws the last step and requires it to exceed each bound at
least three times over, so a change back to last-step drawing fails.

## Checks

```shell
coax/bin/coax test aethyr/loop.coax --library
coax/bin/coax mutate aethyr/loop.coax
coax/bin/coax test verse/player.coax --library --only FPS
```

`aethyr/loop.coax` has nine checks. They cover:

- rounding seconds to nanoseconds;
- refusing and clamping a frame's time;
- the limit on steps in one frame;
- paying the steps that ran;
- a long pause;
- `α`;
- simulation time in seconds;
- a clock read from zeroed memory;
- the step count at four frame rates.

`coax mutate` makes 74 mutants of `aethyr/loop.coax`, and the checks kill all
74.

## What it replaced

Before this module, each realm kept its own accumulator:

- `verse/player.coax`: f32 seconds, 1/64 s, eight steps, 0.25 s.
- Orbit League: f64, 1/120 s, at most eight steps of carried time.
- Dead Gears' ECS clock in
  [verse/deadgears/ecs.coax](../../verse/deadgears/ecs.coax): f64, 1/60 s, a
  0.1 s limit, eight steps, and `1e-7` of slack so that 0.05 s counts as
  three steps.
- `verse/demolish.coax`, Everglade's debris: f64, 1/60 s, 0.25 s, six steps.
- Hold the Stair: f32, 1/60 s, 0.1 s of play times the speed, and a guard of
  64 steps a frame.
- The page faces of the haunted house, Fairy Forest, the Garden, Everglade
  and the villa: a 0.1 s clamp in C before the player stepped.
- The realm template, Star Catcher
  ([verse/realm-template/starcatch.coax](../../verse/realm-template/starcatch.coax)),
  which every new realm copies, and Watchtower: f32, 1/60 s, 0.1 s, and a
  guard of 16 steps a frame.
- Ice Cream Man's rules
  ([verse/icecream/play.coax](../../verse/icecream/play.coax)): f32, 1/30 s,
  0.1 s, at most four steps, and the whole backlog thrown away at the cap.
- The Verse landing's orbs
  ([apps/openagents-web/verse.coax](../../apps/openagents-web/verse.coax)):
  f32, 1/60 s, at most 0.1 s owed.

All of them now step on `aethyr/loop.coax`. Dead Gears' clock keeps its
1/60 s step as 16,666,666 ns, rounded down, so 0.05 s is still three steps
without the slack. The faces move their realm by `verse_player_counts`, the
time the player's clock counts the frame for (`loop_counts`), so the frame's
animation and the player's steps are clamped by one clock.

No frame runs more steps than a realm's budget of 8
([chapter 16](ch016-budgets-that-refuse.md)):

| Realm | Step | Clamp | Most steps a frame |
| --- | --- | --- | --- |
| Walking realms (the player) | 1/64 s | 0.25 s | 8 |
| Orbit League | 1/120 s | 0.1 s | 8 (it was 12) |
| Dead Gears' rules | 1/60 s | 0.05 s | 3 |
| Everglade's debris | 1/60 s | 0.25 s | 6 |
| Hold the Stair | 1/60 s | 0.1 s of play times the speed | 8 (it was 64) |
| The realm template and Watchtower | 1/60 s | 0.1 s | 6 (it was 16) |
| Ice Cream Man's rules | 1/30 s | 0.1 s | 4 |
| The Verse landing's orbs | 1/60 s | 0.1 s | 6 |

At 8x, Hold the Stair runs at full speed down to 60 FPS. Below that, and in
Orbit League below 15 FPS, the time past the cap is dropped and counted on
the clock. Everglade runs two clocks a frame, the player's and the debris',
so a stalled frame there runs up to 14 steps in all.

Each of these realms has a check that feeds it a stalled 3 s frame and
asserts it runs no more than its cap. Ice Cream Man's cap keeps the part of a
step the dropped backlog leaves, as every clock does, where its own
accumulator threw it all away.

## Dependencies

This is the book's first chapter. From Coax, you need checks and mutation
([Coax chapter 11](../../coax/book/ch11-writing-checks.md)) and determinism
([Coax chapter 16](../../coax/book/ch16-determinism.md)). Later, R13 typed
clocks and multirate ticks ([Coax roadmap](../../coax/ROADMAP.md)) give
simulation time its own type. For background, see the
[game-engine systems inventory, section 5](../../docs/research/game-engine-systems.md).
