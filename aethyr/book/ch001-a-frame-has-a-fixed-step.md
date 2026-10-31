# 1. A frame has a fixed step

> Engine-core task: `ec-loop` (#12609), done in `3102756f8b`. The module is
> [aethyr/loop.coax](../loop.coax); the walking controller in
> [verse/player.coax](../../verse/player.coax) (every walking realm) and Orbit
> League step on it.

A display delivers frames when it can: 60 a second on most phones, 120 or 144
on fast screens, and sometimes a 300 ms stall when a tab wakes up. A
simulation that advances by whatever time a frame happened to take gives
different jumps, different landings and different collisions at every frame
rate. Aethyr separates the two clocks. Simulation advances only in fixed
steps; drawing shows a point between the last two steps. One owner advances
time, and nothing else does.

## What this chapter teaches

- Why a fixed step makes physics independent of the frame rate.
- Keeping time in whole nanoseconds, so a step count never depends on float
  rounding.
- Clamping a long frame, capping the steps one frame runs, and dropping the
  backlog past the cap.
- Interpolating the drawn state between the last two steps.
- Checking that a jump looks smooth, with numbers.

## The method

Each frame the host reports the elapsed time `Δ`. The clock turns it into
whole nanoseconds, refuses what is no time, clamps it, adds it to what it
carries, and owes as many whole steps `h` as fit, up to a cap `N_max`. A
backlog past the cap is dropped, keeping the part of a step it leaves. After
the steps run, the part of a step still carried is the drawing factor:

```jai
Δ'    = clamp(round_ns(Δ), 0, Δ_max)    // a stall counts as at most Δ_max
a     = a + Δ'
n     = ⌊a / h⌋
if n > N_max { a = a − (n − N_max) h; n = N_max }   // the backlog is dropped
a     = a − n h                         // after the n steps run: a in [0, h)
α     = a / h                           // the drawing factor, in [0, 1)
draw  = lerp(state_prev, state_now, α)
```

Before each step the realm copies the fields it draws to their `prev_`
copies; after the last step, drawing reads `lerp(prev, now, α)`. Angles mix
the short way round (`wrap_angle(now − prev)`).

Time is an integer. The walking step is 1/64 s, 15,625,000 ns exactly, and 64
steps are exactly one second. A rate that is not a power of two, such as
Orbit League's 1/120 s (8,333,333 ns), still steps deterministically: the
same frame times always give the same step counts, because no float total is
ever compared against a step. Aethyr's defaults are a 1/64 s step, frames
counted as at most 0.25 s, and at most eight steps a frame.

## The clock

The clock is a value. A realm keeps one in its state and replaces it each
frame; it also counts what it refused, clamped and dropped, so frame pacing
can be read and checked.

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
}
```

Accepting a frame does all of the method's arithmetic but the steps
themselves. A clock read from memory that was never constructed (chapter 3)
takes the defaults it was declared with instead of freezing on a zero step,
clamp or cap.

<!-- source: aethyr/loop.coax accept -->
```jai
loop_accept_ns :: (c: FrameClock, frame_ns: int) -> FrameClock #export_c {
  next := c;
  // A clock read from zeroed memory takes the defaults it was declared with.
  if c.step_ns <= 0 { next.step_ns = LOOP_STEP_NS; }
  if c.max_frame_ns <= 0 { next.max_frame_ns = LOOP_MAX_FRAME_NS; }
  if c.max_steps <= 0 { next.max_steps = LOOP_MAX_STEPS; }
  next.frames += 1;
  f := frame_ns;
  if f < 0 { next.refused += 1; f = 0; }
  if f > next.max_frame_ns { next.overlong += 1; f = next.max_frame_ns; }
  next.carry_ns += f;
  next.due = next.carry_ns / next.step_ns;
  if next.due > next.max_steps {
    drop := (next.due - next.max_steps) * next.step_ns;
    next.stalls += 1;
    next.dropped_ns += drop;
    next.carry_ns -= drop;
    next.due = next.max_steps;
  }
  return next;
}
```

The realm runs the steps due, then tells the clock they ran. `loop_alpha` is
what drawing mixes by:

<!-- source: aethyr/loop.coax ran -->
```jai
// How many steps to run now.
loop_due :: (c: FrameClock) -> int #export_c { return c.due; }

// The steps due have run: they are paid, and simulation time moves on by them.
loop_ran :: (c: FrameClock) -> FrameClock #export_c {
  next := c;
  next.carry_ns -= c.due * c.step_ns;
  next.ticks += c.due;
  next.due = 0;
  return next;
}

// One frame in one call: accept its time, and the steps it runs.
FrameTick :: struct { clock: FrameClock; steps: int; }
loop_frame :: (c: FrameClock, frame: f64) -> FrameTick #export_c {
  a := loop_accept(c, frame);
  return FrameTick.{clock = loop_ran(a), steps = a.due};
}

// How far this frame lies from the last step to the next, in [0, 1), once the steps due
// have run: what drawing mixes the last two by.
loop_alpha :: (c: FrameClock) -> f32 #export_c {
  return cast(f32) (cast(f64) (c.carry_ns - c.due * c.step_ns) / cast(f64) c.step_ns);
}
```

## One owner of time

The walking controller is the first client. Its frame entry point accepts the
frame on the player's clock, runs exactly the steps due, keeping where each
began, and pays them; nothing else in the controller reads the frame's time.

<!-- source: verse/player.coax advance -->
```jai
// Advances by a frame's time in fixed steps, on the player's clock (aethyr/loop.coax):
// the frame's time is clamped, at most LOOP_MAX_STEPS run, and a longer backlog is
// dropped, so a stall never makes a frame slower.
advance_in :: (w: World, p: Player, frame: f32) -> Player {
  next := p;
  next.clock = loop_accept(p.clock, cast(f64) frame);
  next = steps_in(w, next, loop_due(next.clock));
  next.clock = loop_ran(next.clock);
  return next;
}
// Runs n fixed steps (what a clock says are due), keeping where the last began, so the
// frame interpolates between the last two.
steps_in :: (w: World, p: Player, n: int) -> Player {
  next := p;
  for i: 0..<n {
    next.prev_pos = next.pos;
    next.prev_target = next.target;
    next.prev_face = next.face;
    next.prev_dist = next.dist;
    next = step(w, next, STEP);
  }
  return next;
}
```

What is drawn reads only the last two steps and `α`: the feet, the facing
and the camera.

<!-- source: verse/player.coax drawn -->
```jai
// The feet and facing to draw this frame.
verse_player_feet :: (p: Player) -> Vec3 #export_c { return mix3(p.prev_pos, p.pos, alpha(p)); }
verse_player_face :: (p: Player) -> f32 #export_c {
  return wrap_angle(p.prev_face + wrap_angle(p.face - p.prev_face) * alpha(p));
}
// The camera this frame: what it looks at, and where it is. A wall pulls the
// camera in at once, so it is never shown further out than the last step.
verse_player_at :: (p: Player) -> Vec3 #export_c { return mix3(p.prev_target, p.target, alpha(p)); }
verse_player_eye :: (p: Player) -> Vec3 #export_c {
  at := verse_player_at(p);
  o := orbit_offset(p);
  d := lo(mix(p.prev_dist, p.dist, alpha(p)), p.dist);
  return Vec3.{x = at.x + o.x * d, y = at.y + o.y * d, z = at.z + o.z * d};
}
```

## Steps are the same at any frame rate

The clock's contract, as a check: fed five seconds at 30, 60, 120 or 144
frames a second, it has run `⌊time / h⌋` steps after every frame, and the
time drawn, `(ticks + α) h`, is the time fed.

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

[aethyr/examples/frame-rates.coax](../examples/frame-rates.coax) feeds the
same five seconds and a stall, and prints what the clock did:

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

A sixtieth of a second rounds up to 16,666,667 ns, so 300 frames feed
5.0000001 s and reach the 320th step; the other rates round down and stop
just short of it. The stall counts as 0.25 s, runs eight steps and drops the
other eight; the frame after it runs one step, as an ordinary frame does.

The player's own check is stronger: a run and jump fed at each of the four
rates passes through the same states, bit for bit, as one step at a time
(`verse/player.coax`, "a run and jump has the same states at 30, 60, 120 and
144 FPS"), and what is drawn always lies between the last two steps.

## A jump has no stutter

At 60 FPS the 64 Hz steps do not land one to a frame: most frames run one
step and some run two. Drawn at the last step, a jump's height lurches each
time two steps land in one frame. Drawn between the last two steps, its
per-frame move changes by little more than gravity changes it.

The check measures how much the drawn height's per-frame move changes from
one frame to the next, through the jump's flight, and bounds it:

<!-- source: verse/player.coax stutter -->
```jai
// No stutter at 60 FPS: the 64 Hz steps do not land one to a frame (some frames run two),
// yet what is drawn moves smoothly. Through a jump's flight the feet drawn each frame change
// speed by no more than gravity does in a frame plus the interpolation's error (measured
// 0.008 m), and the camera by no more than its spring's own pull adds (0.014 m); drawing the
// last step instead lurches by 0.077 m (feet) and 0.062 m (camera) every few frames.
#test "a jump drawn at 60 FPS has no stutter" {
  w: World;
  p := verse_player_input(verse_player_place(0, 0, 0, 0), 0, 1, 0, 0, true, true, 0, 0);
  frame := cast(f32) (1.0 / 60.0);
  bound := f32(GRAVITY) * frame * frame + 0.004;   // 0.0107 m
  eye_bound := 0.016f32;
  was_y := 0.0f32; was_dy := 0.0f32; was_eye := 0.0f32; was_deye := 0.0f32;
  flying := 0;
  for k: 0..<90 {
    p = advance_in(w, p, frame);
    y := verse_player_feet(p).y;
    eye := verse_player_eye(p).y;
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
  q := verse_player_input(verse_player_place(0, 0, 0, 0), 0, 1, 0, 0, true, true, 0, 0);
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

[aethyr/examples/jump-at-60.coax](../examples/jump-at-60.coax) prints the
worst of each over the same jump:

```shell
coax/bin/coax run aethyr/examples/jump-at-60.coax
```

```text
gravity alone changes a frame's move by 6.7 mm
feet: interpolated 7.0 mm, last step 76.2 mm
camera: interpolated 13.1 mm, last step 61.3 mm
```

| Drawn at 60 FPS | Interpolated | Last step | Bound checked |
| --- | ---: | ---: | ---: |
| Feet: change in a frame's move | 7.0 mm | 76.2 mm | 10.7 mm |
| Camera: change in a frame's move | 13.1 mm | 61.3 mm | 16 mm |

Gravity alone accounts for 6.7 mm (`g Δ²`, with `g` 24 m/s² and `Δ` 1/60 s),
so interpolation leaves 0.3 mm of error in the feet. The camera follows on a
spring, which adds its own pull. The same check requires the last-step
drawing to fail the bounds by at least three times, so a regression to it
cannot pass.

## Checks

```shell
coax/bin/coax test aethyr/loop.coax --library
coax/bin/coax mutate aethyr/loop.coax
coax/bin/coax test verse/player.coax --library --only FPS
```

`aethyr/loop.coax` has nine checks: nanoseconds to the nearest, refusal and
clamping, the cap, paying steps, a stall, `α`, simulation time, a zeroed
clock, and the frame-rate contract. `coax mutate` makes 74 mutants of it and
the checks kill all 74.

## What it replaced

Three accumulators with three sets of constants: `verse/player.coax`'s (f32
seconds, 1/64 s, eight steps, 0.25 s), Orbit League's (f64, 1/120 s, a carry
capped at eight steps) and Dead Gears' ECS clock
([verse/deadgears/ecs.coax](../../verse/deadgears/ecs.coax): f64, 1/60 s, a
0.1 s cap, eight steps, a `1e-7` slack so 0.05 s is three steps). The first
two step on `aethyr/loop.coax` now. Dead Gears' clock, Ice Cream Man's
1/30 s step, the 0.1 s frame clamps in six page shells' C glue and
`verse/demolish.coax`'s accumulator move with the realm migration
([chapter 64](ch064-every-realm-runs-on-aethyr.md)).

## Dependencies

None in this book; this is the first piece. From Coax: checks and mutation
([Coax chapter 11](../../coax/book/ch11-writing-checks.md)), determinism
([Coax chapter 16](../../coax/book/ch16-determinism.md)), and later R13 typed
clocks and multirate ticks ([Coax roadmap](../../coax/ROADMAP.md)), which give
the step's simulation time its own type. Background:
[game-engine systems inventory, section 5](../../docs/research/game-engine-systems.md).
