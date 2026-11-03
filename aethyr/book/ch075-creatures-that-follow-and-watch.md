# 75. Creatures that follow and watch

> **Status: partly implemented.** Senses and knowledge are shared:
> [aethyr/ai/stimulus.coax](../../aethyr/ai/stimulus.coax) (`#authority`)
> gives sight, sound and touch as typed stimuli, sight through
> [chapter 7](ch007-ask-the-world-where-things-are.md)'s `query_ray`, and
> keeps a creature's knowledge in a bounded `Memory` that forgets after a
> stated number of ticks and counts what it refuses. Following on the
> realm's fixed step is shared too:
> [aethyr/follower.coax](../../aethyr/follower.coax) steps the grounded
> follower and the fetching dog once per tick of
> [chapter 1](ch001-a-frame-has-a-fixed-step.md)'s clock. The example plays a
> guard and a dog for 480 ticks at 30, 60 and 144 FPS and with uneven frames,
> and the four digests are equal. The flock
> ([aethyr/flock.coax](../../aethyr/flock.coax)) keeps its own f32
> accumulator and reads its target once a frame, so it isn't the same at
> every frame rate; the example shows that. No realm senses through the new
> module yet: Everglade's watch and guards keep their own rules, and
> decisions as data, remote choices and sleep budgets aren't built. See
> [Still to do](#still-to-do).

A creature in a realm does three things each step: it senses, it decides and
it acts. This chapter separates those three, so a guard, a bird and a dog use
the same rules for sight and hearing, decide from what they sensed rather
than from the true state, and replay exactly.

## What this chapter teaches

- How to step creatures at a fixed rate, independent of the frame rate.
- How to model sight and hearing as typed stimuli with ranges and line of
  sight, built on chapter 7's queries.
- How to keep a creature's knowledge separate from the world's truth, as
  [chapter 45](ch045-sensors-that-see-the-past.md) does for spacecraft.
- Where frame rate still leaks in: the flock.

## Senses and stimuli

A creature's `Senses` are its sight range, its eye height above its feet, its
touch reach and how many ticks it remembers a stimulus. The defaults are the
Everglade watch's (`WATCH_SIGHT`, 34 m, and `WATCH_EYE`, 1.6 m, in
`verse/everglade/watch.coax`). A tick is one step of
[aethyr/loop.coax](../../aethyr/loop.coax), 1/64 s, so the default `forget`
of 128 ticks is 2 s. Every stimulus says what kind it is, whom it is of (the
realm's entity number), where, and on which tick:

<!-- source: aethyr/ai/stimulus.coax senses -->
```jai
SENSE_MEMORY :: 8;             // stimuli one creature remembers at most
SENSE_REACH :: 1000.0;         // metres: a longer range is not one
SENSE_EYE_MAX :: 100.0;        // metres: and a higher eye
SENSE_FORGET_MAX :: 230400;    // ticks: an hour at 64 a second

StimulusKind :: enum u8 { none; sight; sound; touch; }
SenseStatus :: enum u8 { sensed; out_of_range; hidden; invalid; }
NoteStatus :: enum u8 { noted; refreshed; invalid; stale; full; }

// One creature's senses. The defaults are the Everglade watch's: WATCH_SIGHT and WATCH_EYE.
Senses :: struct {
  sight: f32 = 34;     // metres, flat, it sees
  eye: f32 = 1.6;      // metres its eyes are above its feet
  touch: f32 = 0.6;    // metres, flat, that counts as a touch
  forget: int = 128;   // ticks a stimulus is remembered: 2 s
}
// What was sensed: what kind, of whom (the realm's entity number), where, and on which tick.
Stimulus :: struct { kind: StimulusKind; source: int; at: WorldPoint; tick: int; }
Sensed :: struct { status: SenseStatus; stimulus: Stimulus; }
```

`senses_ok` holds each field to a range: sight and touch from 0 to
`SENSE_REACH` (1,000 m), the eye from 0 to 100 m, and `forget` from 1 tick to
an hour. Senses outside it sense nothing and say `invalid`.

### Sight

Sight is the watch's rule: near enough on the flat (`watch_sees` uses
`flat_gap`), and nothing between the eyes and the point seen. The line of
sight is one `query_ray` from the eye to the target with `far` 1, so the
direction is the whole segment and a hit before t = 1 means something is in
the way. Boxes, capsules and heightfields all hide, because they are all in
chapter 7's `Colliders`. A ray that starts inside a box hits it at t = 0, so
an eye inside a wall sees nothing. A query the colliders refuse (more than
16 heightfields, a broken mesh, a search that ran out) senses nothing and
says `invalid`, not `hidden`:

<!-- source: aethyr/ai/stimulus.coax sight -->
```jai
// Whether a creature standing at `feet` sees `target` (a point on someone, such as their chest)
// on tick `now`: near enough on the flat, and nothing between its eyes and the point. A query the
// colliders refuse (too many heightfields, a broken mesh) senses nothing and says so.
sense_sight :: (s: Senses, c: Colliders, feet: WorldPoint, target: WorldPoint, source: int, now: int) -> Sensed {
  out: Sensed;
  if !senses_ok(s) || !sense_point_ok(feet) || !sense_point_ok(target) || now < 0 { out.status = SenseStatus.invalid; return out; }
  if sense_flat(feet, target) > s.sight { out.status = SenseStatus.out_of_range; return out; }
  eye := world_point(feet.x, feet.y + s.eye, feet.z);
  hit := query_ray(c, eye, world_sub(target, eye), 1);
  if hit.status != QueryStatus.ok { out.status = SenseStatus.invalid; return out; }
  if hit.hit && hit.t < 1 { out.status = SenseStatus.hidden; return out; }
  out.stimulus = Stimulus.{kind = StimulusKind.sight, source = source, at = target, tick = now};
  return out;
}
```

### Sound

A sound carries a range given by its source, as the watch's `WATCH_HEAR`
gives 85 m for the meteors and 22 m for the orb's splash. Walls don't stop
it:

<!-- source: aethyr/ai/stimulus.coax sound -->
```jai
// Whether a creature at `feet` hears a sound made at `at` that carries `range` metres. Walls don't
// stop a sound here, as they don't in the watch.
sense_sound :: (s: Senses, feet: WorldPoint, at: WorldPoint, range: f32, source: int, now: int) -> Sensed {
  out: Sensed;
  if !senses_ok(s) || !sense_point_ok(feet) || !sense_point_ok(at) || !sense_range_ok(range) || now < 0 { out.status = SenseStatus.invalid; return out; }
  if sense_flat(feet, at) > range { out.status = SenseStatus.out_of_range; return out; }
  out.stimulus = Stimulus.{kind = StimulusKind.sound, source = source, at = at, tick = now};
  return out;
}
```

Touch is the same test with the creature's own `touch` reach (0.6 m).

## Knowledge, not truth

What a creature knows is its `Memory`: at most `SENSE_MEMORY` (8) stimuli,
one for each source and kind. A new sighting of someone already seen
refreshes that entry in place. Before a new entry goes in, everything older
than `forget` ticks is forgotten. A stimulus is refused, and counted in
`refused`, when it is broken (kind `none`, a position that isn't a number, a
negative tick), stamped before the newest tick already noted (`stale`), or
when all eight places are taken (`full`):

<!-- source: aethyr/ai/stimulus.coax memory -->
```jai
// What one creature knows: at most SENSE_MEMORY stimuli, one per source and kind, each forgotten
// `forget` ticks after it was sensed. Stamps never go back: `latest` is the newest tick noted.
Memory :: struct {
  items: [SENSE_MEMORY] Stimulus;
  count: int;
  latest: int = -1;
  refused: int;        // stimuli it could not take: broken, out of order, or no room
  forgotten: int;      // stimuli it has let go
}
Noted :: struct { memory: Memory; status: NoteStatus; }

// The memory on tick `now`, with every stimulus older than `forget` ticks gone. The rest keep
// their order. A tick before the newest one noted changes nothing.
memory_forget :: (m: Memory, s: Senses, now: int) -> Memory {
  if now < m.latest || !senses_ok(s) { return m; }
  out := m; out.count = 0;
  for i: 0..<m.count {
    if now - m.items[i].tick > s.forget { out.forgotten += 1; continue; }
    out.items[out.count] = m.items[i]; out.count += 1;
  }
  return out;
}

// The memory with one more stimulus. One of the same kind from the same source is refreshed in
// place; a new one goes at the end, after the old ones are forgotten. Refused, and counted, when
// it is broken, stamped before the newest tick, or there is no room.
memory_note :: (m: Memory, s: Senses, st: Stimulus) -> Noted {
  out := Noted.{memory = m};
  if st.kind == StimulusKind.none || !sense_point_ok(st.at) || st.tick < 0 || !senses_ok(s) {
    out.memory.refused += 1; out.status = NoteStatus.invalid; return out;
  }
  if st.tick < m.latest { out.memory.refused += 1; out.status = NoteStatus.stale; return out; }
  out.memory = memory_forget(m, s, st.tick);
  out.memory.latest = st.tick;
  for i: 0..<out.memory.count {
    if out.memory.items[i].kind == st.kind && out.memory.items[i].source == st.source {
      out.memory.items[i] = st; out.status = NoteStatus.refreshed; return out;
    }
  }
  if out.memory.count >= SENSE_MEMORY { out.memory.refused += 1; out.status = NoteStatus.full; return out; }
  out.memory.items[out.memory.count] = st; out.memory.count += 1;
  return out;
}
```

A decision reads `memory_recall`. It gives the newest remembered stimulus of
a source and its age in ticks, never the source's true position. On the same
tick a sight wins over a sound, and a sound over a touch:

<!-- source: aethyr/ai/stimulus.coax recall -->
```jai
// What the creature knows of `source` on tick `now`: its newest remembered stimulus of them (on a
// tie, sight before sound before touch), and how many ticks old it is. A decision reads this, not
// where the source really is.
Recall :: struct { known: bool; kind: StimulusKind; at: WorldPoint; age: int; }
memory_recall :: (m: Memory, s: Senses, source: int, now: int) -> Recall {
  out: Recall;
  if now < m.latest || !senses_ok(s) { return out; }
  best := -1;
  for i: 0..<m.count {
    one := m.items[i];
    if one.source != source || now - one.tick > s.forget { continue; }
    if best < 0 || one.tick > m.items[best].tick || (one.tick == m.items[best].tick && cast(int) one.kind < cast(int) m.items[best].kind) { best = i; }
  }
  if best < 0 { return out; }
  return Recall.{known = true, kind = m.items[best].kind, at = m.items[best].at, age = now - m.items[best].tick};
}
```

Everglade's wanted state already works this way in its own code: out of
every hunting guard's sight for `WANTED_LOSE` (18 s), you are no longer
wanted.

## A guard and a wall, worked by hand

The guard stands at the origin with the default senses. A wall is the box
from (−3, 0, 9) to (3, 3, 10). You walk along z = 20 from x = −12 at 3 m/s,
which is 0.046875 m a tick (3 / 64, exact in binary), so on tick k you are at
x = −12 + 0.046875 k. The guard looks at your chest, 1.2 m up.

1. **Range.** The farthest you get is x = −12 at the start: √(12² + 20²) =
   23.3 m, inside 34 m. You stay in range the whole walk.
2. **The line.** From the eye (0, 1.6, 0) to your chest (x, 1.2, 20), the
   line reaches the wall's near face, z = 9, at t = 9 / 20 = 0.45. There it
   is at height 1.6 − 0.4 × 0.45 = 1.42 m, inside the wall's 0 to 3 m, and at
   x′ = 0.45 x. It leaves the far face, z = 10, at t = 0.5 and x′ = 0.5 x,
   farther out. So the wall hides you exactly when |0.45 x| < 3, that is
   |x| < 6.667 m.
3. **Hidden.** On tick 113, x = −6.703125 and 0.45 x = −3.016: seen. On tick
   114, x = −6.65625 and 0.45 x = −2.995: hidden. You come out on tick 399,
   x = 6.703125, by the same numbers.
4. **Forgetting.** The last sighting is tick 113. It is kept while
   now − 113 ≤ 128, so through tick 241, and forgotten on tick 242.
5. **Hearing.** On tick 300 you knock a crate over at x = −12 + 14.0625 =
   2.0625. Its sound carries 22 m. The guard is √(2.0625² + 20²) = 20.106 m
   away and hears it, so it knows where you are again, from the sound,
   while the wall still hides you.

On tick 200 the guard's memory still holds the sighting from tick 113,
87 ticks old: x = −6.703125. You are really at x = −12 + 9.375 = −2.625,
behind the wall. A guard that walks to what it knows walks to where you
were.

## Following on the fixed step

[aethyr/follower.coax](../../aethyr/follower.coax) is the dog of Hecate's
Cemetery on the realm's clock. `follower_step` takes one tick of
`follower_step_seconds()` (1/64 s) and never a frame's time. It walks at
2.6 m/s, runs at 5.2 m/s, accelerates at 14 m/s² and turns at 9 a second
(`follower_profile`), goes round scenery boxes and stands on the sampled
ground. `follower_frame` admits a frame only when the realm's clock says how
many ticks it ran, and refuses a duplicate or skipped frame. Its checks and
its oracle (`aethyr/tests/follower-oracle.coax`, the older variable-time
kernel fed one tick) are recorded in
[follower-qualification.json](../../aethyr/tests/follower-qualification.json).

In the example the dog starts at (−8, 24) and follows you, stopping
`FOLLOW_STOP` (2.2 m) short, with the wall as a scenery block for its 0.4 m
body.

## The example

[aethyr/examples/watch-and-follow.coax](../examples/watch-and-follow.coax)
plays the guard and the dog. Each tick you move, the guard looks (and on
tick 300 listens), notes what it sensed, forgets what is too old and recalls
you, and the dog takes one step. The frames come from `loop_frame`; each
frame runs the ticks it is owed, so the frame rate decides only how many
ticks a frame runs:

<!-- source: aethyr/examples/watch-and-follow.coax example -->
```jai
// FNV-1a over 32-bit words, kept in 32 bits.
mix32 :: (h: int, v: int) -> int { return ((h ^ (v & 4294967295)) * 16777619) & 4294967295; }
mixf :: (h: int, x: f32) -> int { return mix32(h, cast(int) f32_bits(x)); }

Scene :: struct {
  senses: Senses; memory: Memory; seen: bool; known: bool;
  dog: Follower; tick: int;
}
you_at :: (k: int) -> WorldPoint { return world_point(-12 + f32(WALK) * f32(k), 0, 20); }

// One fixed step: you walk, the guard looks (and on tick 300 hears you knock a crate over), the
// dog follows. `say` prints what changes.
step :: (s: Scene, c: Colliders, g: mesh.Ground, h: [] f32, wall: [] f32, say: bool) -> Scene {
  n := s; n.tick = s.tick + 1; k := n.tick;
  feet := you_at(k); chest := world_point(feet.x, 1.2, feet.z);
  look := sense_sight(n.senses, c, world_point(0, 0, 0), chest, YOU, k);
  if look.status == SenseStatus.sensed { n.memory = memory_note(n.memory, n.senses, look.stimulus).memory; }
  else { n.memory = memory_forget(n.memory, n.senses, k); }
  if k == 300 {
    heard := sense_sound(n.senses, world_point(0, 0, 0), feet, 22, YOU, k);
    n.memory = memory_note(n.memory, n.senses, heard.stimulus).memory;
    if say { print("tick 300: a crate falls at x = %; heard: %\n", feet.x, heard.status == SenseStatus.sensed); }
  }
  seen := look.status == SenseStatus.sensed;
  r := memory_recall(n.memory, n.senses, YOU, k);
  if say && seen != s.seen { print("tick %: you at x = %, %\n", k, feet.x, ifx seen then "seen" else "hidden by the wall"); }
  if say && r.known != s.known {
    if r.known { print("tick %: the guard knows where you are (%)\n", k, ifx r.kind == StimulusKind.sight then "sight" else "sound"); }
    else { print("tick %: the guard has forgotten you\n", k); }
  }
  if say && k == 200 { print("tick 200: the guard would go to x = % (% ticks old); you are at x = %\n", r.at.x, r.age, feet.x); }
  n.seen = seen; n.known = r.known;
  empty: [1] f32; links: [1] int;
  d := follower_step(n.dog, follower_profile(), g, h, empty[0..<0], links[0..<0], 0.9, v3(feet.x, 0, feet.z), f32(FOLLOW_STOP), wall, f32(DOG_BODY));
  if d.status == FollowerStatus.ok { n.dog = d.follower; }
  return n;
}

digest :: (s: Scene) -> int {
  h := 2166136261;
  h = mixf(h, s.dog.pos.x); h = mixf(h, s.dog.pos.y); h = mixf(h, s.dog.pos.z);
  h = mixf(h, s.dog.vel.x); h = mixf(h, s.dog.vel.z); h = mixf(h, s.dog.face);
  for i: 0..<s.memory.count {
    one := s.memory.items[i];
    h = mix32(h, cast(int) one.kind); h = mix32(h, one.tick); h = mixf(h, one.at.x); h = mixf(h, one.at.z);
  }
  return mix32(mix32(h, s.memory.refused), s.memory.forgotten);
}

// Plays the ticks with frames of `a` and `b` seconds in turn; returns the digest after tick 480.
play :: (a: f64, b: f64, say: bool) -> int {
  wall := [6] f32.[-3, 0, 9, 3, 3, 10]; c: Colliders; c.boxes = wall[..];
  g := mesh.Ground.{n = 1, cell = 100, west = -50, south = -50}; h: [4] f32;
  s: Scene; s.dog = verse_follower_place(g, h[..], -8, 24, 0);
  t: clock.FrameClock; frames := 0;
  while s.tick < TICKS {
    f := clock.loop_frame(t, ifx frames % 2 == 0 then a else b); t = f.clock; frames += 1;
    for i: 0..<f.steps { if s.tick < TICKS { s = step(s, c, g, h[..], wall[..], say); } }
  }
  if say { print("after % ticks (% frames): the dog at (%, %), the guard remembers % stimulus\n", s.tick, frames, s.dog.pos.x, s.dog.pos.z, s.memory.count); }
  return digest(s);
}
```

The refusals: a ninth source, a stamp before the newest, a target that isn't
a number, and senses that forget after 0 ticks.

<!-- source: aethyr/examples/watch-and-follow.coax refused -->
```jai
refusals :: () {
  s: Senses; m: Memory;
  for i: 0..<SENSE_MEMORY { m = memory_note(m, s, Stimulus.{kind = StimulusKind.sound, source = 10 + i, tick = 5}).memory; }
  full := memory_note(m, s, Stimulus.{kind = StimulusKind.sound, source = 99, tick = 5});
  print("a ninth source on tick 5: %, % refused, % kept\n", full.status, full.memory.refused, full.memory.count);
  stale := memory_note(full.memory, s, Stimulus.{kind = StimulusKind.sound, source = 10, tick = 4});
  print("a stamp of tick 4 after tick 5: %, % refused\n", stale.status, stale.memory.refused);
  nan := 0.0f32; nan = nan / nan;
  c: Colliders;
  print("sight of a target at x = NaN: %\n", sense_sight(s, c, world_point(0, 0, 0), world_point(nan, 1, 1), YOU, 6).status);
  bad := s; bad.forget = 0;
  print("senses that forget after 0 ticks: %\n", sense_sound(bad, world_point(0, 0, 0), world_point(1, 0, 0), 5, YOU, 6).status);
}
```

Its output, the same in debug and release:

```text
tick 1: you at x = -11.953125, seen
tick 1: the guard knows where you are (sight)
tick 114: you at x = -6.65625, hidden by the wall
tick 200: the guard would go to x = -6.703125 (87 ticks old); you are at x = -2.625
tick 242: the guard has forgotten you
tick 300: a crate falls at x = 2.0625; heard: true
tick 300: the guard knows where you are (sound)
tick 399: you at x = 6.703125, seen
after 480 ticks (450 frames): the dog at (5.5974574, 20.035662), the guard remembers 1 stimulus
digest at 60 FPS: 475816136
the same at 30 and 144 FPS and with 5 and 25 ms frames: true
a ninth source on tick 5: full, 1 refused, 8 kept
a stamp of tick 4 after tick 5: stale, 2 refused
sight of a target at x = NaN: invalid
senses that forget after 0 ticks: invalid
the flock after 2 s at 30.0 FPS: 1.9999982 s flown, bird 0 at x = 5.167675, the same as at 60: false
the flock after 2 s at 60.0 FPS: 1.9999982 s flown, bird 0 at x = 5.1427135, the same as at 60: true
the flock after 2 s at 144.0 FPS: 1.9999982 s flown, bird 0 at x = 5.138543, the same as at 60: false
```

The ticks printed are the ones worked by hand above: hidden from 114,
forgotten on 242, heard on 300 and seen again on 399. The four runs (60 FPS,
30 FPS, 144 FPS, and frames of 5 ms and 25 ms in turn) give the same digest
of the dog and the guard's memory after 480 ticks, because neither reads a
frame's time.

The flock is different. `flock_frame` adds the frame's `dt` to an f32
`pending` and steps at `FLOCK_STEP` (1/120 s), at most `FLOCK_CATCH_UP` (30)
steps a frame. Every step within a frame reads the target where it was at the
start of the frame. With a target walking at 3 m/s, its first bird ends 2 s
at x = 5.1677 at 30 FPS, 5.1427 at 60 and 5.1385 at 144. With a target
standing still the three rates agree. Moving the flock onto loop.coax's clock
would change Everglade's companion, so it is left as it is (see below).

## Checks

- `coax/bin/coax test aethyr/ai/stimulus.coax --library`: 109 of 109 (13 of
  the module's own and its oracle's, and the 96 of `query.coax` and the
  modules it loads). The module's checks cover sight in range, behind the
  wall, past the range, on the flat at 34 m and 34.01 m, over a 1.5 m wall
  with the eye at 1.6 m and not at 1.4 m, a target on the wall's face, an
  eye inside it, and every refusal; sound and touch at and just past their
  ranges; noting, refreshing, forgetting at tick 148 and 149, ordering after
  forgetting; and every refusal of the memory.
- The independent check, `aethyr/tests/stimulus-oracle.coax`, walks each
  segment in 1,000 even samples and asks whether a sample is strictly inside
  a box. It shares no code with `query_ray`. Of 300 random scenes of three
  boxes, it judges 292 (the other 8 graze an edge: the boxes grown and shrunk
  by 5 cm disagree), 39 hidden and 253 seen, and `sense_sight` agrees on all
  292.
- `coax/bin/coax mutate aethyr/ai/stimulus.coax --library --root-only`: 150
  mutants, 150 killed.
- `python3 psionic/tests/book.py --book aethyr --coax coax/bin/coax`: the
  example's output in debug and release equals
  `aethyr/fixtures/watch-and-follow.out`.

## An external actor: observed controls

An actor can also live in another engine. The Verse OS WoW adapter cannot
query WoW through Aethyr's colliders or step its physics. It observes a
screenshot and a visible read-only public-API sensor strip instead.
`aethyr/ai/pilot.coax` gives that adapter bounded actions built from
chapter 4's `Intent`, and records them in chapter 5's `ReplayEvent`
envelope. A 300 ms forward action has `move_y = 1`; its record carries
the action name and 300 ms. A 501 ms action is refused. Turn intents carry
the requested turn direction; the external adapter calibrates its key pulse
and reobserves the actual heading, rather than promising a physical angle.

`os/game-navigate.coax` is the native consumer. It captures, takes one
decision, rechecks the sensor, acquires a short desk lease, acts, releases,
waits for rendering, then captures again. Psionic's `GdContext` ticket binds
a local Elephant choice to that exact observation and window lifetime. The
record replays the controller's accepted event sequence; it cannot reproduce
WoW's server or promise the same resulting world.

These external controls are separate from the in-engine creature example.
They do not complete this chapter's general behavior graphs, path planning
or realm-wide model adoption. The [navigation journal](../../docs/os/verse-navigation.md)
records real routes, refused input and model trials.

## Still to do

- **Realms on the module.** Everglade's watch
  (`verse/everglade/watch.coax`: `watch_sees`, `WATCH_HEAR`, the wanted
  state) and its guards still sense in their own code. Moving them changes
  Everglade, which the owner is changing now.
- **The flock on the fixed step.** `aethyr/flock.coax` on loop.coax's ticks,
  reading its target per tick, with Everglade's companion golden unchanged.
- **Decisions as data.** States and transitions (a guard's beat, turning on
  you, `GUARD_RUN`, `SWING_EVERY`) and utility choices, with every input
  recorded and each decision's rule kept for inspection.
- **A check against reading truth.** Today a decision reads the memory by
  convention; nothing refuses one that reads true positions.
- **Remote choices.** Admission of a decision model's answer only as an enum
  value, at most one request pending a realm, and a late answer refused for a
  guard that is no longer calm
  ([pylon-guard-decisions.md](../../docs/verse/pylon-guard-decisions.md),
  gated today by [verse/test_guard_ai.sh](../../verse/test_guard_ai.sh)).
- **Budgets and sleep.** Far creatures stepping less often or sleeping, with
  rates declared per realm and counted by
  [chapter 16](ch016-budgets-that-refuse.md).
- **A recorded fight** replaying to the same digest
  ([chapter 5](ch005-replay-a-session-exactly.md)), and the same digests in
  the browser.

## What exists today

- [aethyr/ai/stimulus.coax](../../aethyr/ai/stimulus.coax): senses, stimuli
  and memory, used by this chapter's example.
- [aethyr/follower.coax](../../aethyr/follower.coax): the follower and the
  fetching dog on the realm's clock, used by Hecate's Cemetery
  ([apps/openagents-web/villa.coax](../../apps/openagents-web/villa.coax)).
- [aethyr/flock.coax](../../aethyr/flock.coax): the following flock, used by
  Everglade's bird companion (`eg-companion`) and the made-realm `flock`
  feature.
- [verse/everglade/watch.coax](../../verse/everglade/watch.coax): the town
  watch, crimes, sight and hearing, wanted state and SRD 5.2.1 guards.
- [verse/everglade/guard-decision.coax](../../verse/everglade/guard-decision.coax)
  and [guard-types.coax](../../verse/everglade/guard-types.coax): the
  cosmetic guard decision, its observation (`GaObservation`) and effects.
- [docs/research/game-engine-systems.md](../../docs/research/game-engine-systems.md),
  section 37: the perception, knowledge and decision inventory.

## Acceptance checks and oracles

The chapter is done when:

- The flock's existing `#test`s pass on a fixed-step follow, and Everglade's
  companion looks the same in its golden.
- A creature behaves the same at 30, 60 and 144 FPS, bit for bit (met for
  the follower and the senses; not for the flock).
- A guard never reacts to a crime it could not see or hear, checked over a
  recorded session with walls between.
- A behaviour that reads true state instead of its memory fails a check.
- The guard decision gate passes in debug, release and sanitized modes, and
  a remote answer outside the enum, or for a guard that is no longer calm,
  is refused and counted.
- A recorded fight replays to the same digest.

## Dependencies

Chapters 1 (the clock), 5 (replay), 6 (entities), 7 (queries), 16 (budgets)
and 45 (truth and observation kept apart). Psionic's public decision edge
([Psionic chapter 27](../../psionic/book/ch027-a-public-mesh.md)) for remote
choices. Coax R13 typed clocks for stimulus stamps.
