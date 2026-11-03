# 56. The Grid

> **Status: partly implemented.** The Grid's session,
> [verse/gridwalk.coax](../../verse/gridwalk.coax), is built from the
> foundation chapters: chapter 1's clock, chapter 4's typed actions and the
> touch sticks, chapter 9's walking controller over the Grid's declared
> collision profile, chapter 10's camera follow, and chapter 5's replay,
> which records every frame and the digest of every step. The page at
> `/realms/the-grid` runs it in Wasm through its C face
> ([newland-face.c](../../apps/openagents-web/newland-face.c)) and draws
> through chapter 11's shared renderer; the recorded route from the
> homepage's WALK AROUND is qualified headless
> ([route record](../../docs/verification/aethyr-grid-playable-route-2026-11-02.md)).
> A recorded session replays to the same step digests natively and in the
> browser. The example below runs natively in debug and release. Not built
> yet: the floor drawn from one world-space grid as a checked property,
> other people as chapter 6 entities, and the homepage's scene reduced to
> the Grid's content. See [Still to do](#still-to-do).

The Grid is the smallest whole realm: a floor of lines in a black field, a
figure that walks, runs and jumps on it, a camera behind, other people
walking, and stations to walk up to. It is the first realm to rebuild
because it uses almost every foundation chapter and almost nothing else, so
any seam in the foundations shows here first: the floor that shifts as you
move, the jump that stutters, the feet drawn above the ground.

## What this chapter teaches

- Composing a realm's session from the foundations: one clock, typed
  actions, the walking controller, the camera, replay.
- Keys and two thumbs saying the same thing as one `Intent` per frame.
- A session that records itself and replays exactly, with the first step
  that differs found when it doesn't.

## The session

A `GridWalk` is everything the Grid steps: you (chapter 9's `Player`, whose
clock is chapter 1's), the input buffer the steps read (chapter 4), the
phone's thumbs, and the record (chapter 5) with the state it starts from:

<!-- source: verse/gridwalk.coax gridwalk-state -->
```jai
GridWalk :: struct {
  profile: int = GW_PROFILE;
  you: pc.Player;
  input: InputBuffer;        // what the steps read (aethyr/input.coax)
  thumbs: Sticks;            // the phone's fingers (aethyr/sticks.coax)
  prev_theta: f32;           // the camera's turn when the last step began, for drawing
  prev_phi: f32;
  // the record (aethyr/replay.coax) and the state it starts from
  start: pc.Player;
  start_input: InputBuffer;
  events: [GW_EVENTS] ReplayEvent;
  marks: [GW_MARKS] int;
  channels: [REPLAY_SOURCES] ReplayChannel;
  n: [REPLAY_COUNTERS] int;
  records: int;              // records started, the first included
  // the view the touches are measured in, and the time they are stamped with
  width: f32; height: f32; dpr: f32 = 1;
  now: f64;
}
```

## One frame

Once a frame the page's face hands the session what the devices said and
the frame's time. The keys and the touch stick become one `Intent`; that
intent is one event in the record; then the steps the frame's time makes due
run. A frozen frame (the page's golden pictures) runs no step and records
nothing:

<!-- source: verse/gridwalk.coax gridwalk-frame -->
```jai
// This frame's controls as one Intent: the touch stick when it is pushed past the dead zone,
// else the keys; a jump from Space or a tap; the drag's, the arrows' and the right thumb's look
// together; the wheel's and the pinch's zoom (already in metres, `zoom`).
gw_intent :: (g: [] GridWalk, key_x: f32, key_y: f32, shift: bool, space: bool, look_x: f32, look_y: f32, zoom: f32) -> Intent {
  t := g[0].thumbs;
  acting := false;   // the Grid has no act control yet
  keys := input_keys(key_x, key_y, shift, space, acting, look_x + t.dyaw, look_y + t.dpitch, zoom);
  stick := input_stick(t.pad_x, t.pad_y, t.run, t.jump, acting, 0, 0, 0);
  i := keys;
  if stick.move_x != 0 || stick.move_y != 0 { i.move_x = stick.move_x; i.move_y = stick.move_y; i.run = stick.run; }
  i.jump = keys.jump || stick.jump;
  i.mended += stick.mended;
  return i;
}

// Once a frame: the controls into the record, then the steps the frame's time makes due. A
// frozen frame (the page's golden pictures) runs no step and records nothing.
gridwalk_frame :: (g: [] GridWalk, key_x: f32, key_y: f32, shift: bool, space: bool, look_x: f32, look_y: f32, zoom: f32,
                   width: f32, height: f32, dpr: f32, dt: f32, frozen: bool) #export_c {
  if g.count < 1 { return; }
  g[0].width = width; g[0].height = height; g[0].dpr = dpr;
  g[0].now += cast(f64) pc.hi(ifx pc.finite(dt) then dt else 0, 0);
  if frozen { g[0].thumbs = verse_sticks_used(g[0].thumbs); return; }
  s := gw_session(g);
  clock := loop_accept(g[0].you.clock, cast(f64) dt);
  steps := loop_due(clock);
  if replay_events(s) >= GW_EVENTS || replay_tick(s) + steps > GW_MARKS {
    gw_take(g, s);   // frames still waiting for a step go into the new record's start
    gw_begin(g);
  }
  e := replay_frame_event(s, gw_intent(g, key_x, key_y, shift, space, look_x, look_y, zoom), steps);
  v := replay_offer(s, e);
  g[0].thumbs = verse_sticks_used(g[0].thumbs);
  for k: 0..<steps { gw_step(g); }
  g[0].you.clock = loop_ran(clock);
}
```

## One step

Each fixed step, live or replayed, takes the frames the record holds for it,
samples the input buffer once, and runs one step of the walking controller
in the Grid's world (its declared collision profile, `the-grid`). The step
leaves its state's digest in the record:

<!-- source: verse/gridwalk.coax gridwalk-step -->
```jai
// The frames the record holds for the next step, into the input buffer.
gw_take :: (g: [] GridWalk, s: ReplaySession) {
  while true {
    t := replay_take(s);
    if !t.ok { break; }
    g[0].input = input_frame(input_feed(g[0].input, replay_intent(t.event)), replay_steps(t.event));
  }
}
// One fixed step, live or replayed: the frames the record holds for it, then one sample of the
// input buffer, then the controller's step.
gw_world :: () -> pc.World {
  return pc.World.{collision = walk_profiles.collision_profile_lookup("the-grid").profile};
}
gw_step :: (g: [] GridWalk) {
  s := gw_session(g);
  gw_take(g, s);
  sm := input_sample(g[0].input);
  g[0].input = sm.buffer;
  a := sm.actions;
  p := pc.walk_input(g[0].you, 0, 0, a.move_x, a.move_y, a.run, false, a.look_x, a.look_y);
  if a.jump { p.jumps = input_press(p.jumps); }
  p = pc.walk_zoom(p, a.zoom);
  g[0].prev_theta = g[0].you.theta; g[0].prev_phi = g[0].you.phi;
  w := gw_world();
  g[0].you = pc.steps_in(w, p, 1);
  replay_stepped(s, gridwalk_digest(g));
}
```

Drawing reads the session and never steps it: the feet, the facing and the
camera's eye between the last two steps (chapter 10's
[smooth follow](ch010-the-camera-follows.md) is the Grid's check that the eye
moves without a jump at 60 FPS).

## Replay

The latest record replays from where it began into a second session. It is
true only when every step gives the recorded digest and every frame is taken
at its step:

<!-- source: verse/gridwalk.coax gridwalk-replay -->
```jai
// Replays the latest record from where it began into `again`, which must not be the session
// itself: true when every step gave the recorded digest and every frame was taken at its step.
// The Grid execution profile is checked before writing `again`; ReplayEvent's envelope stays
// unchanged. Old profile-1 digests cannot be interpreted as profile 2.
gridwalk_replay :: (g: [] GridWalk, again: [] GridWalk) -> bool #export_c {
  if g.count < 1 || again.count < 1 || g[0].profile != GW_PROFILE { return false; }
  again[0] = g[0];
  again[0].you = g[0].start;
  again[0].input = g[0].start_input;
  s := gw_session(again);
  if !replay_play(s, gridwalk_digest(again)) { return false; }
  while replay_tick(s) < s.n[REPLAY_MARKS] { gw_step(again); }
  return replay_matched(s);
}
// The record's step digests, for comparing hosts: step k's, or -1 past the last.
gridwalk_mark :: (g: [] GridWalk, k: int) -> int #export_c {
  if g.count < 1 || k < 0 || k >= g[0].n[REPLAY_MARKS] { return -1; }
  return g[0].marks[k];
}
gridwalk_steps :: (g: [] GridWalk) -> int #export_c { return ifx g.count < 1 then 0 else g[0].n[REPLAY_MARKS]; }
gridwalk_session :: (g: [] GridWalk) -> int #export_c { return ifx g.count < 1 then 0 else replay_digest(gw_session(g)); }
```

### Worked by hand

The example below plays 240 frames at 60 FPS: the forward key held
throughout, Shift (run) for the first 90 frames, Space on frames 120 and 190.

- **Steps.** 240 frames of 1/60 s are 4 s. At 64 steps a second that is
  4 × 64 = 256 steps.
- **A jump.** The controller leaves the ground at `JUMP` = 7.5 m/s against
  `GRAVITY` = 24 m/s² ([aethyr/walk.coax](../walk.coax)). Continuously, the
  peak is 7.5² / (2 × 24) = 1.17 m and the time in the air is
  2 × 7.5 / 24 = 0.625 s, 37.5 frames. Two jumps keep the feet off the floor
  for about 75 frames. Fixed 1/64 s steps and drawing between steps give a
  little less: 73 frames, and a highest drawn foot of 1.11 m, the
  1.11328125 m the route record measured.
- **The first divergent step.** Frame 100 starts at 100/60 = 1.667 s. By
  then floor(1.667 × 64) = 106 steps have run, so frame 100's first step is
  step 106 (counting from 0). Change what frame 100 recorded and the replay
  first differs there.

## Walk the Grid

The program is
[aethyr/examples/walk-the-grid.coax](../examples/walk-the-grid.coax). It
places you where the Grid's page does, at (28, 28) facing the places'
middle:

<!-- source: aethyr/examples/walk-the-grid.coax example -->
```jai
#load "../../verse/gridwalk.coax";
#import "Basic";

cm :: (m: f32) -> int { return cast(int) (m * 100 + ifx m < 0 then -0.5f32 else 0.5f32); }

// The Grid's own session, as the page's face drives it: placed at (28, 28) facing the places'
// middle, then 240 frames at 60 FPS of the forward key, Shift held for the first 90 and Space on
// frames 120 and 190. Then the record replays from its start into a second session, and again
// with one frame's forward push changed.
main :: () {
  g: [1] GridWalk; g[0] = GridWalk.{};
  gridwalk_place(g[..], 28, 28, -2.35619449, 1.35, 9);
  top := 0.0f32; air := 0;
  for f: 0..<240 {
    gridwalk_frame(g[..], 0, 1, f < 90, f == 120 || f == 190, 0, 0, 0, 1000, 2000, 2, 1.0 / 60.0, false);
    y := gridwalk_feet(g[..]).y;
    if y > 0 { air += 1; }
    if y > top { top = y; }
  }
  feet := gridwalk_feet(g[..]);
  print("frames 240, steps %, jumps %, frames with the feet off the floor %, highest % cm\n",
        gridwalk_steps(g[..]), g[0].you.jumps.taken, air, cm(top));
  print("feet at (% cm, % cm, % cm), on the ground: %\n", cm(feet.x), cm(feet.y), cm(feet.z), g[0].you.on_ground);
  print("session digest %\n", gridwalk_session(g[..]));
  again: [1] GridWalk;
  print("replayed from the record's start: %, same digest: %\n", gridwalk_replay(g[..], again[..]),
        gridwalk_session(again[..]) == gridwalk_session(g[..]));
  k := 100;
  g[0].events[k].words[1] = replay_bits(-1);   // frame 100's forward push, recorded as backward
  print("frame % changed: replays %, first divergent step %\n", k, gridwalk_replay(g[..], again[..]),
        replay_diverged(gw_session(again[..])));
}
```

```shell
coax/bin/coax run aethyr/examples/walk-the-grid.coax
```

```text
frames 240, steps 256, jumps 2, frames with the feet off the floor 73, highest 111 cm
feet at (1124 cm, 0 cm, 1124 cm), on the ground: true
session digest 3036777944
replayed from the record's start: true, same digest: true
frame 100 changed: replays false, first divergent step 106
```

The steps, the jumps, the frames in the air, the highest foot and the
divergent step are the hand-worked ones. You walk along the diagonal, so
x and z stay equal. The last line is the broken case: one frame's recorded
push, changed, is caught at the step where that frame's input is first read.

## Checks

- `coax/bin/coax test verse/gridwalk.coax --library` (and `coax mutate
  verse/gridwalk.coax --root-only`, `#authority`): placing, recording,
  keyboard and stick reaching the same digests, the dead zone and taps, turns
  equal at 30, 60 and 144 FPS, the camera drawn between steps, replay and
  its fault, records that fill or are blurred starting new ones that replay,
  the digest's fields, and the camera following at 60 FPS without a jump.
- The independent check is the browser. `coax/web/tool.sh replay-hosts grid`
  ([replay-hosts.coax](../../coax/web/replay-hosts.coax)) builds
  [verse/tests/grid-session.coax](../../verse/tests/grid-session.coax), 1,000
  frames at 60, 120 and 144 FPS with a stall, keys, a drag, the wheel, the
  stick, a pinch and taps, to Wasm the way pages are built, runs it in
  headless, muted Chrome and compares every step's digest with the native
  run.
- The [route record](../../docs/verification/aethyr-grid-playable-route-2026-11-02.md)
  qualifies the page itself: the homepage's WALK AROUND, 240 frames with two
  jumps, the camera, replay, a resource refusal and recovery, and a final
  320×180 frame matching the CPU reference, WebGPU and Vulkan.

## What else the Grid draws today

- [verse/grid.coax](../../verse/grid.coax): the bare world as aethyr-cpp's
  `grid.cpp` drew it (lines every 4 m to 264 m, street lines midway between
  24 m lots, fading from 6 m to 110 m, a ring at your feet, the Pylon
  terminal 4 m ahead), drawn from the touch sticks of
  [aethyr/sticks.coax](../sticks.coax).
- [apps/openagents-web/newland-walk.coax](../../apps/openagents-web/newland-walk.coax)
  over [newland.coax](../../apps/openagents-web/newland.coax): the homepage's
  places, trees, roads, Pylons and people walked; a click opens the
  homepage's card. The face sends your place to the playtest room
  `the-grid` about five times a second, and the room's answer is everyone
  else.
- In the browser it draws through chapter 11's shared renderer: its page
  names the `the-grid` package ([page-render.coax](../render/page-render.coax)),
  and every GPU object has a ledger handle
  ([registry.coax](../render/registry.coax)). A lost GPU device keeps its
  session ([gpu-recovery.coax](../../coax/web/gpu-recovery.coax)).
- Its edit rows (walkers, names) are in
  [realm-edit.md](../../docs/verse/realm-edit.md).

## Still to do

- **The floor as one world-space grid, checked.** A line at world
  `x = 24` m must project to the same place for the same camera whether you
  stand at the origin or 200 m away. `verse/grid.coax` draws its lines in
  world space; there is no check of that property yet.
- **People as entities.** Everyone else from the room becomes a chapter 6
  entity with a stable ID; today the page keeps the room's answer itself
  ([newland-network.coax](../../apps/openagents-web/newland-network.coax)).
- **The Grid's own content.** `newland-walk.coax` is still the homepage's
  scene walked, with its own grid-unit conversion (`WALK_SCALE`, half a grid
  unit a metre); reduce it to the Grid's content and HUD, loading `aethyr/`
  for everything else.
- **What the route record leaves open:** the interactive `vk-realm` window
  and its full 240-frame lifecycle, touch, live network, accessibility and
  final matching-main toolchain acceptance.

### Acceptance checks still open

- The floor's world-space check above.
- The figure's feet at its physics height on every drawn frame of the page
  (the route record checks the drawn feet for the recorded route).
- The page's golden (`coax/web/goldens/`) changes only where a fix is
  visible; its checks pass
  (`coax test apps/openagents-web/newland.coax --steps 400000000`).

## Dependencies

Chapters 1–11, 14, 15 and 49.
