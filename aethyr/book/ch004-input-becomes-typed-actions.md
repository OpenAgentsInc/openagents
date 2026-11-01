# 4. Input becomes typed actions

> Engine-core task: `ec-input` (#12609). The module is
> [aethyr/input.coax](../input.coax). The walking controller in
> [verse/player.coax](../../verse/player.coax) takes its jump from it, and
> its run rule and dead zone come from it.

Input arrives as events at any time: keys, mouse moves, touches, and pad
axes. The simulation runs in fixed steps ([chapter 1](ch001-a-frame-has-a-fixed-step.md)).
Between the two, Aethyr turns the events into a small set of typed actions:
move, look, zoom, run, jump, and act. Each step reads one sample of these
actions.

Without this layer, each realm reads devices its own way. A phone stick and
a keyboard can then disagree about running. A quick tap can also be lost
between two frames that run no step, or counted twice in a frame that runs
two.

## What this chapter teaches

- How to separate actions from the devices that produce them.
- The difference between a level (something held) and a press (something
  that happens once).
- How to give each press to exactly one step.
- How to split a look movement across a frame's steps.
- How to handle dead zones, out-of-range values, and values that aren't
  numbers.

## The method

Aethyr handles three kinds of input:

- **Levels:** the move axes and run. A step reads whatever is held when it
  runs.
- **Presses:** jump and act. Aethyr counts each press when it happens. The
  first step that samples after the press takes it, and no other step does.
- **Amounts:** look and zoom. Aethyr adds them up between steps. The steps of
  one frame split the total equally, so the total turn doesn't depend on the
  frame rate.

Each frame, the host and the realm do the following:

1. Convert the device state to an `Intent` with `input_keys` or
   `input_stick`.
2. Pass the `Intent` to `input_feed`.
3. Call `input_frame` with the number of steps that the clock says are due.
4. Before each step, call `input_sample` and read its `Actions`.

## The rules

These constants and functions set the rules that every device follows:

- A stick's push inside `INPUT_DEAD_ZONE` (0.2) is no move.
- If a stick is past the dead zone, its full push decides whether you run.
  Otherwise, the keys decide: moving runs by default, and Shift walks
  (`INPUT_RUN_BY_DEFAULT`).
- A value that isn't a number, is infinite, or is 1,000,000 or more is no
  input.
- An axis is clamped to [-1, 1].

`InputAxis.mended` and `Intent.mended` count the values that Aethyr refused
or clamped, so a device that sends bad values shows up in the counts.

<!-- source: aethyr/input.coax rules -->
```jai
INPUT_DEAD_ZONE :: 0.2;          // stick travel that counts as no input
INPUT_RUN_BY_DEFAULT :: true;    // the owner's rule (2026-10-29): moving runs, Shift walks
INPUT_MAX_WAITING :: 4;          // presses one edge keeps for later steps; more are counted, not kept
INPUT_TOO_BIG :: 1000000.0;      // an amount this large is not one

// A number that is a number: not infinite, not NaN, not absurd. Anything else is no input.
input_finite :: (a: f32) -> bool { return a == a && a < INPUT_TOO_BIG && a > -INPUT_TOO_BIG; }
// An axis in [-1, 1], and whether the device's number had to be mended (refused as no
// number, or clamped from past full travel): counted, so a device that sends junk shows.
InputAxis :: struct { value: f32; mended: int; }
input_axis :: (a: f32) -> InputAxis {
  if !input_finite(a) { return InputAxis.{value = 0, mended = 1}; }
  if a > 1 { return InputAxis.{value = 1, mended = 1}; }
  if a < -1 { return InputAxis.{value = -1, mended = 1}; }
  return InputAxis.{value = a};
}
// Whether you run: a stick past the dead zone decides by its push; the keys go by
// INPUT_RUN_BY_DEFAULT, Shift turning it the other way.
input_keys_run :: (shift: bool) -> bool { return shift != INPUT_RUN_BY_DEFAULT; }
input_runs :: (shift: bool, stick_run: bool, pad_x: f32, pad_y: f32) -> bool #export_c {
  if input_finite(pad_x) && input_finite(pad_y) && sqrt(pad_x * pad_x + pad_y * pad_y) > INPUT_DEAD_ZONE { return stick_run; }
  return input_keys_run(shift);
}
```

## Presses

An `InputPress` counts presses and how many a step has taken:

- `input_press` records a press event, such as a key going down or a tap.
- `input_level` takes the held state once a frame. A change from not held to
  held counts as one press. Use it when the host only reports what is held.
- `input_take` gives one waiting press to a step.

At most `INPUT_MAX_WAITING` (4) presses wait. Aethyr counts any press past
that in `lost` and doesn't keep it. At 64 steps a second, five presses
within one step's 15.6 ms don't happen in normal play.

<!-- source: aethyr/input.coax press -->
```jai
// A press-type control: held now, and presses not yet taken by a step.
InputPress :: struct {
  held: bool;
  waiting: int;     // presses waiting for a step, at most INPUT_MAX_WAITING
  pressed: int;     // presses seen
  taken: int;       // presses a step took
  lost: int;        // presses past INPUT_MAX_WAITING: counted, never kept
}
// A press event (a key going down, a tap), whatever was held.
input_press :: (e: InputPress) -> InputPress {
  next := e;
  next.held = true;
  next.pressed += 1;
  if e.waiting < INPUT_MAX_WAITING { next.waiting += 1; } else { next.lost += 1; }
  return next;
}
input_release :: (e: InputPress) -> InputPress { next := e; next.held = false; return next; }
// A level seen once a frame (a face that only knows what is held): a rise is a press.
input_level :: (e: InputPress, down: bool) -> InputPress {
  if down && !e.held { return input_press(e); }
  next := e; next.held = down;
  return next;
}
// A step takes one waiting press, if there is one.
InputTake :: struct { edge: InputPress; pressed: bool; }
input_take :: (e: InputPress) -> InputTake {
  if e.waiting <= 0 { return InputTake.{edge = e, pressed = false}; }
  next := e; next.waiting -= 1; next.taken += 1;
  return InputTake.{edge = next, pressed = true};
}
```

## One intent from any device

An `Intent` is one frame's controls. `input_keys` builds it from the keyboard,
and `input_stick` builds it from the touch stick or a pad. Both apply the
same rules, so the same play gives the same `Intent`.

<!-- source: aethyr/input.coax intent -->
```jai
// One frame's controls, from any device: the move axes (x right, y forward, each in
// [-1, 1]), run, jump and act held, and look (radians: + turns left and looks up) and zoom
// (steps) asked since the last frame.
Intent :: struct {
  move_x: f32; move_y: f32;
  run: bool; jump: bool; act: bool;
  look_x: f32; look_y: f32; zoom: f32;
  mended: int;      // numbers refused or clamped on the way in
}
// Look and zoom: any finite amount, or none.
input_amounts :: (i: Intent, look_x: f32, look_y: f32, zoom: f32) -> Intent {
  next := i;
  if input_finite(look_x) { next.look_x = look_x; } else { next.mended += 1; }
  if input_finite(look_y) { next.look_y = look_y; } else { next.mended += 1; }
  if input_finite(zoom) { next.zoom = zoom; } else { next.mended += 1; }
  return next;
}
// The keyboard: axes of -1, 0 or 1 from the keys held.
input_keys :: (key_x: f32, key_y: f32, shift: bool, jumping: bool, acting: bool, look_x: f32, look_y: f32, zoom: f32) -> Intent {
  x := input_axis(key_x); y := input_axis(key_y);
  i := Intent.{move_x = x.value, move_y = y.value, run = input_keys_run(shift), jump = jumping, act = acting, mended = x.mended + y.mended};
  return input_amounts(i, look_x, look_y, zoom);
}
// The touch stick (verse/grid.coax Sticks) or a pad: a push inside the dead zone is no move,
// a full push runs, a quick tap jumps. A bad number on either axis is no push at all.
input_stick :: (pad_x: f32, pad_y: f32, full_push: bool, tap: bool, acting: bool, look_x: f32, look_y: f32, zoom: f32) -> Intent {
  x := input_axis(pad_x); y := input_axis(pad_y);
  i := Intent.{run = input_runs(false, full_push, pad_x, pad_y), jump = tap, act = acting, mended = x.mended + y.mended};
  if input_finite(pad_x) && input_finite(pad_y) && sqrt(x.value * x.value + y.value * y.value) > INPUT_DEAD_ZONE {
    i.move_x = x.value; i.move_y = y.value;
  }
  return input_amounts(i, look_x, look_y, zoom);
}
```

## The buffer and its samples

An `InputBuffer` holds what the steps read: the levels last fed, the presses
waiting, and the look and zoom not yet taken. `input_sample` returns one
step's `Actions` and the updated buffer. If a realm samples more steps than
it declared with `input_frame`, the extra sample takes all that is left, and
Aethyr counts it in `untimed`.

`input_blur` handles a lost focus. It releases every level at once, and keeps
the presses that were already made.

<!-- source: aethyr/input.coax buffer -->
```jai
// What the steps read: levels as last fed, edges waiting, and look and zoom gathered.
InputBuffer :: struct {
  move_x: f32; move_y: f32;
  run: bool;
  jump: InputPress; act: InputPress;
  look_x: f32; look_y: f32; zoom: f32;   // asked and not yet taken by a step
  steps_left: int;                       // steps this frame will still sample (input_frame)
  mended: int;                           // numbers refused or clamped, all told
  untimed: int;                          // samples past the steps a frame declared: each took all
}
// What one step reads.
Actions :: struct {
  move_x: f32; move_y: f32;
  run: bool; jump: bool; act: bool;
  look_x: f32; look_y: f32; zoom: f32;
}

// Once a frame: levels replaced, a rise counted as a press, amounts gathered.
input_feed :: (b: InputBuffer, i: Intent) -> InputBuffer {
  x := input_axis(i.move_x); y := input_axis(i.move_y);
  a := input_amounts(Intent.{}, i.look_x, i.look_y, i.zoom);
  next := b;
  next.move_x = x.value; next.move_y = y.value;
  next.run = i.run;
  next.jump = input_level(b.jump, i.jump);
  next.act = input_level(b.act, i.act);
  next.look_x += a.look_x; next.look_y += a.look_y; next.zoom += a.zoom;
  next.mended += i.mended + x.mended + y.mended + a.mended;
  return next;
}
// The steps this frame runs (loop_due): the amounts gathered are split among them.
input_frame :: (b: InputBuffer, steps: int) -> InputBuffer {
  next := b;
  next.steps_left = steps;
  return next;
}
// One step's sample: levels as held, one waiting press per edge, and this step's share of
// the amounts. A sample past the steps the frame declared takes all of them, and is counted.
Sampled :: struct { buffer: InputBuffer; actions: Actions; }
input_sample :: (b: InputBuffer) -> Sampled {
  next := b;
  j := input_take(b.jump); k := input_take(b.act);
  next.jump = j.edge; next.act = k.edge;
  parts := b.steps_left;
  if parts < 1 { parts = 1; next.untimed += 1; } else { next.steps_left -= 1; }
  a := Actions.{move_x = b.move_x, move_y = b.move_y, run = b.run, jump = j.pressed, act = k.pressed,
    look_x = b.look_x / f32(parts), look_y = b.look_y / f32(parts), zoom = b.zoom / f32(parts)};
  // the last share is all that is left, so nothing gathered outlives its frame
  next.look_x -= a.look_x; next.look_y -= a.look_y; next.zoom -= a.zoom;
  return Sampled.{buffer = next, actions = a};
}
// Focus lost: every level is let go at once; presses already made still count.
input_blur :: (b: InputBuffer) -> InputBuffer {
  next := b;
  next.move_x = 0; next.move_y = 0; next.run = false;
  next.jump = input_release(b.jump); next.act = input_release(b.act);
  return next;
}
```

The example [aethyr/examples/one-press.coax](../examples/one-press.coax)
feeds a tap that is shorter than a frame, then runs frames of zero, two, and
one steps:

<!-- source: aethyr/examples/one-press.coax example -->
```jai
#load "../input.coax";
#import "Basic";

// A tap shorter than a frame, fed to frames that run 0, 2 and 1 steps: the press waits for
// the first step and only that step jumps. A 0.3 rad drag is split over the frame's steps.
main :: () {
  b: InputBuffer;
  b.jump = input_press(b.jump);
  b.jump = input_release(b.jump);
  b = input_feed(b, input_keys(0, 1, false, false, false, 0.3, 0, 0));
  frames := [3] int.[0, 2, 1];
  n := 0;
  for f: 0..<3 {
    b = input_frame(b, frames[f]);
    print("frame %, steps %\n", f, frames[f]);
    for s: 0..<frames[f] {
      x := input_sample(b);
      b = x.buffer;
      print("  step %: jump %, run %, move (%, %), look %\n", n, x.actions.jump, x.actions.run, x.actions.move_x, x.actions.move_y, x.actions.look_x);
      n += 1;
    }
  }
  print("presses % taken % waiting % lost %\n", b.jump.pressed, b.jump.taken, b.jump.waiting, b.jump.lost);
  k := input_keys(0, 1, false, true, false, 0, 0, 0);
  t := input_stick(0, 1, true, true, false, 0, 0, 0);
  print("keyboard and stick agree: %\n", k.move_x == t.move_x && k.move_y == t.move_y && k.run == t.run && k.jump == t.jump);
}
```

```shell
coax/bin/coax run aethyr/examples/one-press.coax
```

```text
frame 0, steps 0
frame 1, steps 2
  step 0: jump true, run true, move (0.0, 1.0), look 0.15
  step 1: jump false, run true, move (0.0, 1.0), look 0.15
frame 2, steps 1
  step 2: jump false, run true, move (0.0, 1.0), look 0.0
presses 1 taken 1 waiting 0 lost 0
keyboard and stick agree: true
```

The press waits through frame 0, which runs no step. Step 0 takes it, and
step 1 doesn't. The two steps of frame 1 split the 0.3 rad look.

## The walking controller

`verse_player_input` is the controller's entry point for one frame's
controls. It records a jump as a press with `input_level`:

<!-- source: verse/player.coax input -->
```jai
// This frame's controls, as InputIntent carries them: keyboard axes (-1, 0,
// 1), stick axes in [-1, 1] (x right, y forward), run and jump held, and look
// deltas in radians (positive turns left and looks up). A jump is a press, not a hold:
// it is counted here and taken by the first fixed step after it (aethyr/input.coax), so a
// frame that runs no step keeps it and a frame that runs two jumps once. The look turns the
// camera at once (the realms' cameras read theta and phi each frame; ec-camera moves it).
verse_player_input :: (p: Player, key_x: f32, key_y: f32, pad_x: f32, pad_y: f32,
                          run: bool, jump: bool, dyaw: f32, dpitch: f32) -> Player #export_c {
  next := p;
  next.key_x = clamp(key_x, -1, 1);
  next.key_y = clamp(key_y, -1, 1);
  next.pad_x = clamp(pad_x, -1, 1);
  next.pad_y = clamp(pad_y, -1, 1);
  if !finite(key_x) { next.key_x = 0; }
  if !finite(key_y) { next.key_y = 0; }
  if !finite(pad_x) || !finite(pad_y) { next.pad_x = 0; next.pad_y = 0; }
  next.run = run;
  next.jumps = input_level(p.jumps, jump);
  next.jump_held = jump;
  cam := follow_look(follow_of(next), dyaw, dpitch);
  next.theta = cam.theta; next.phi = cam.phi;
  return next;
}
```

Each fixed step first takes a waiting press. If you are on the ground, you
jump:

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

Before this change, `verse_player_input` applied the jump when it was called,
once a frame. The states that the steps pass through are the same as before:
the jump still starts at the next step. What changed is that a press now
belongs to a step, which replay ([chapter 5](ch005-replay-a-session-exactly.md))
needs.

The controller's look still turns the camera when `verse_player_input` is
called, not per step. The realms' cameras read `theta` and `phi` every frame.
The camera chapter ([chapter 10](ch010-the-camera-follows.md)) moves look to
the steps, together with drawing the camera between the last two steps.

## Checks

The main check compares the buffer with a separate model over 64 random
runs. Each run has 200 frames. A frame has zero, one, or two presses, and
runs zero, one, or two steps, or eight steps like a long pause. The model
gives each press, in order, to the first step that samples after it. It
counts a press that finds four presses already waiting as lost. The buffer
must give each press to the same step as the model.

<!-- source: aethyr/input.coax oracle -->
```jai
#forall "every press is taken by exactly one step, the one the model names" seed: 0..<64 {
  rng := seed * 7919 + 12345;   // a small LCG on int, mod 2^31
  b: InputBuffer;
  ran := 0;             // steps run so far
  waiting := 0;         // the model's queue
  presses := 0; taken := 0; lost := 0;
  for frame: 0..<200 {
    rng = (rng * 1103515245 + 12345) % 2147483648;
    taps := (rng / 65536) % 3;           // 0, 1 or 2 presses this frame
    if (rng / 8192) % 5 != 0 { taps = 0; }
    for t: 0..<taps {
      b.jump = input_press(b.jump); b.jump = input_release(b.jump); presses += 1;
      if waiting < INPUT_MAX_WAITING { waiting += 1; } else { lost += 1; }
    }
    rng = (rng * 1103515245 + 12345) % 2147483648;
    steps := (rng / 131072) % 3;          // 0, 1 or 2 steps
    if (rng / 1024) % 23 == 0 { steps = 8; }        // now and then a stall's eight
    b = input_frame(b, steps);
    for s: 0..<steps {
      x := input_sample(b);
      b = x.buffer;
      expect := waiting > 0;
      if expect { waiting -= 1; }
      #assert x.actions.jump == expect;
      if x.actions.jump { taken += 1; }
      ran += 1;
    }
  }
  #assert b.jump.lost == lost && taken + b.jump.waiting + lost == presses && b.jump.waiting == waiting && b.jump.taken == taken;
  #assert presses > 10 && ran > 100;
}
```

The controller's checks cover these cases:

- A press in a frame that runs no step jumps on the next step.
- A frame that runs two steps jumps once.
- The step that jumps counts as in the air, so on ice it pushes you with full
  grip, as it did before.
- A session on the keyboard and the same session on the touch stick give the
  same `Intent` every frame, and the player passes through the same states.

<!-- source: verse/player.coax presses -->
```jai
// The input contract (aethyr/input.coax, #12609): a press is taken by exactly one step. At
// 144 FPS most frames run no step: a press in one of them jumps on the next step, not never.
// A frame that runs two steps jumps once, not twice.
#test "a press in a frame with no step jumps on the next one, and a frame of two jumps once" {
  w: World;
  p := verse_player_place(0, 0, 0, 0);
  p = verse_player_input(p, 0, 0, 0, 0, false, true, 0, 0);
  p = advance_in(w, p, 0.001);                    // runs no step
  #assert p.clock.ticks == 0 && p.on_ground && p.jumps.waiting == 1;
  p = verse_player_input(p, 0, 0, 0, 0, false, false, 0, 0);   // let go before the step
  p = advance_in(w, p, 0.015);
  #assert p.clock.ticks == 1 && !p.on_ground && p.jumps.taken == 1;
  q := verse_player_place(0, 0, 0, 0);
  q = verse_player_input(q, 0, 0, 0, 0, false, true, 0, 0);
  q = advance_in(w, q, 0.032);                    // two steps
  #assert q.clock.ticks == 2 && q.jumps.taken == 1 && q.jumps.waiting == 0;
  one := step(w, step(w, verse_player_input(verse_player_place(0, 0, 0, 0), 0, 0, 0, 0, false, true, 0, 0), STEP), STEP);
  #assert same_step(q, one) && !same_step(q, step(w, q, STEP)) && !same_step(q, verse_player_place(0, 0, 0, 0));
  // The step that jumps already counts as in the air: on ice, it pushes you with full grip.
  ice := verse_player_place(0, 0, 0, 0); ice.grip = 0.5;
  dry := verse_player_place(0, 0, 0, 0);
  a := step(w, verse_player_input(ice, 0, 1, 0, 0, true, true, 0, 0), STEP);
  b := step(w, verse_player_input(dry, 0, 1, 0, 0, true, true, 0, 0), STEP);
  #assert !a.on_ground && same_step(a, b);
}

// The same action stream drives desktop and phone: a session on the keyboard and the same
// session on the touch stick (forward, running, a jump, a turn to the left) become the same
// Intent each frame, and the player passes through the same states.
#test "the keyboard and the touch stick drive the same session to the same states" {
  w: World;
  k := verse_player_place(0, 0, 0, 0);
  t := verse_player_place(0, 0, 0, 0);
  for f: 0..<120 {
    jump := f == 10 || f == 70;
    x := ifx f >= 40 && f < 60 then -1.0f32 else 0.0f32;
    keys := input_keys(x, 1, false, jump, false, 0, 0, 0);
    touch := input_stick(x, 1, true, jump, false, 0, 0, 0);
    k = advance_in(w, verse_player_intent(k, keys), 1.0 / 60.0);
    t = advance_in(w, verse_player_intent(t, touch), 1.0 / 60.0);
    #assert same_step(k, t) && k.pos.x == t.pos.x;
  }
  #assert k.jumps.taken == 2 && abs(k.pos.z) > 5 && abs(k.pos.x) > 0.5;   // it ran, turned and jumped twice
}
```

```shell
coax/bin/coax test aethyr/input.coax --library
coax/bin/coax mutate aethyr/input.coax
coax/bin/coax test verse/player.coax --library
```

`aethyr/input.coax` has nine checks. They cover:

- clamping, refusing, and counting bad values;
- the run rule and the dead zone;
- counting and taking presses, and the waiting limit;
- the keyboard and the touch stick giving the same `Intent`;
- feeding the buffer and sampling it;
- splitting look and zoom across a frame's steps;
- releasing every level when focus is lost;
- the model check above;
- the total look of a drag at 30, 60, 120, and 144 FPS.

`coax mutate` makes 126 mutants of `aethyr/input.coax`, and the checks kill all
126.

## Menus, focus and keyboard activation

> **Design draft.** Not implemented. [Chapter 15](ch015-ui-and-the-hud.md)
> uses these actions for its focus order and its keyboard and screen-reader
> journey.

The actions above drive a character. A menu, a dialog or the hotbar needs a
second set that works without a pointer:

| Action | Kind | Keyboard | Pad | Screen reader |
| --- | --- | --- | --- | --- |
| `focus_next` | press | Tab, Down, Right | D-pad down or right | swipe right |
| `focus_previous` | press | Shift+Tab, Up, Left | D-pad up or left | swipe left |
| `activate` | press | Enter, Space | A | double-tap |
| `back` | press | Escape | B | two-finger scrub |
| `menu` | press | Escape when no UI has focus | Start | magic tap |

The rules:

1. **UI actions are presses.** They go through the same buffer as `jump`
   and `act`, so each one is taken by exactly one step.
2. **Focus captures input.** While a UI element has focus, presses that the UI
   uses aren't delivered as gameplay actions in the same step. Pressing Space
   on a focused button activates the button and doesn't make the character
   jump. Levels such as the move axes are zero while a modal UI is open.
3. **Activation is one action for every device.** A click, a tap, Enter,
   Space, the pad's A button and a screen reader's activation all produce
   `activate` on the focused element, in the same step.
4. **Remapping covers UI actions.** The remapping of
   [chapter 86](ch086-everyone-can-play.md) can rebind every UI action, but
   it refuses a mapping that leaves `focus_next`, `activate` or `back`
   without a key.

Checks for these actions:

- Space on a focused button activates it once, and the character doesn't
  jump in that step.
- `activate` from the keyboard, the pad and a click on the same button
  produce the same action in the same step.
- A remapping that removes the last key for `back` is refused.

## Still to do

The realm migration ([chapter 64](ch064-every-realm-runs-on-aethyr.md)) moves
the other input paths onto this module:

- the Grid's touch controls (`Sticks` in [verse/grid.coax](../../verse/grid.coax));
- Ice Cream Man's held-key bits (`H_W`, `H_SPACE`, and the others) in
  `verse/icecream/play.coax`;
- the page shells' C code, which can then report key and tap events with
  `input_press` instead of a held level, so that a key pressed and released
  between two frames still counts;
- `openagents.engine.v1.InputIntent` in `proto/`, for remote and agent
  input.

## Dependencies

This chapter uses the fixed step from
[chapter 1](ch001-a-frame-has-a-fixed-step.md) and the spaces from
[chapter 2](ch002-spaces-and-units.md): look is in radians, and pointers are
in screen space. From Coax, R13 clocks and sample-and-hold
([Coax roadmap](../../coax/ROADMAP.md)) and protocol messages
([Coax chapter 18](../../coax/book/ch18-protocol.md)) come later. For
background, see the
[game-engine systems inventory, section 34](../../docs/research/game-engine-systems.md).
