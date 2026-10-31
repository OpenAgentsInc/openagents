# 3. State starts from its defaults

> Engine-core task: `ec-state` (#12609), done in `c5fb95a184`. The browser
> path is [coax/web/defaults.py](../../coax/web/defaults.py), its check is
> [coax/web/state-defaults.test.ts](../../coax/web/state-defaults.test.ts),
> and [aethyr/loop.coax](../loop.coax)'s clock is safe when it was never
> constructed.

On 2026-10-31 Orbit League ran without gravity for about two and a half
hours: its state declared `spin_g: f64 = 1.0`, the native build applied it,
and the browser read `0` because WebAssembly memory starts zeroed. A realm's
state is a value the engine constructs, and the same construction must happen
on every host, before the first step, with nothing left to chance.

## What this chapter teaches

- What a declared default is, and where Coax applies it.
- Why zeroed memory is not a constructed value.
- One construction on native and browser hosts, from the compiler's own
  initializers.
- Comparing initial states byte for byte, and checking that the comparison
  is sharp.

## The method

A realm's state type `T` has, for each field, either a declared default or
the type's zero. Construction is a function of the type alone:

```jai
init(T) = T.{ f₁ = default₁ or zero(f₁), …, fₙ = defaultₙ or zero(fₙ) }
state   = init(T)          // before any step, on every host
```

Natively, `s: [1] T` runs it: the compiler emits `static T coax_init_t_T(void)`
for each struct with a default and calls it. The walking controller's state
leans on defaults: it stands on the ground, its camera sits 4.4 m out behind
and above, its pace and grip are one, and its clock (chapter 1) has its step,
clamp and cap.

<!-- source: verse/player.coax state -->
```jai
Player :: struct {
  pos: Vec3;            // feet, after the last step
  vel: Vec3;
  face: f32;            // yaw the character faces
  on_ground: bool = true;
  act: Act = Act.idle;
  // The orbit camera: theta around +Y from +Z, phi down from +Y, and its
  // distance from the spring-followed target.
  theta: f32 = PI;
  phi: f32 = BEHIND_PHI;
  dist: f32 = CAM_MAX;
  max_dist: f32 = CAM_MAX;
  zoom_push: f32;       // zoom past a view's end, gathering to cross it
  target: Vec3 = Vec3.{x = 0, y = LOOK_AT_HEIGHT, z = 0};
  spring_vel: Vec3;
  // The controls held now.
  key_x: f32;
  key_y: f32;
  pad_x: f32;
  pad_y: f32;
  run: bool;
  jump_held: bool;
  // Fixed steps (aethyr/loop.coax, the engine's one clock): time not yet stepped,
  // and where the last step began, so the frame shows a point between the last two.
  clock: FrameClock;
  prev_pos: Vec3;
  prev_target: Vec3 = Vec3.{x = 0, y = LOOK_AT_HEIGHT, z = 0};
  prev_face: f32;
  prev_dist: f32 = CAM_MAX;
  // Your speed's share, set by the realm: 1 on land, 0.5 wading or swimming
  // (verse/everglade/river.coax pace_of). WALK and RUN themselves never change.
  pace: f32 = 1;
  grip: f32 = 1;       // the share of your feet's grip on the ground (a realm's ice: everglade-frost.coax); less slides you
  sliding: bool;       // on a floor too steep to stand on, sliding down it
}
```

In the browser the host allocates the state in the module's memory, which is
zeroed, and hands it to the engine. Every default there reads as zero: the
player starts in the air with its camera at the target, and a clock with no
step. So the host now constructs it too, with the compiler's own initializers:

- The build runs `coax/web/defaults.py` over the generated C. It gives each
  `coax_init_t_T` the face's state needs a public name and writes a wrapper
  unit that exports `coaxial_state_init(state)`, which writes every defaulted
  part of a fresh state, and `coaxial_state_default(i)`, where the `i`-th
  defaulted part sits, for the check. A constructor run by `_initialize`
  defaults the face's Coax globals (the player, the thumbs).
- [coax/web/host.js](../../coax/web/host.js) calls `coaxial_state_init` on a
  fresh state, before the first frame.

The code that constructs is the compiler's, the same on both hosts: nothing
restates a default by hand.

## A clock that can't freeze

A value that is constructed in one place can still be read from memory that
wasn't, by a host that predates the export or a state restored from old
bytes. The clock is the piece whose zero is worst: a zero clamp counts every
frame as no time, so nothing ever steps. `loop_accept_ns` takes the declared
default for a step, clamp or cap that is zero or negative
([chapter 1's listing](ch001-a-frame-has-a-fixed-step.md#the-clock)).
[aethyr/examples/state-defaults.coax](../examples/state-defaults.coax) shows
both:

<!-- source: aethyr/examples/state-defaults.coax example -->
```jai
#load "../../verse/player.coax";
#import "Basic";

// A player constructed natively takes its declared defaults. WebAssembly memory starts
// zeroed instead, which coaxial_state_init fixes before the first frame. The clock is also
// safe on its own: read from zeroed memory (no step, no clamp, no cap), it takes its
// declared defaults on its first frame and steps as a constructed one does.
main :: () {
  made: [1] Player;
  p := made[0];
  print("constructed: on_ground %, camera % m out at phi %, pace %, grip %\n", p.on_ground, p.dist, p.phi, p.pace, p.grip);
  print("constructed clock: step % ns, clamp % ns, at most % steps\n", p.clock.step_ns, p.clock.max_frame_ns, p.clock.max_steps);
  zeroed := p.clock;
  zeroed.step_ns = 0; zeroed.max_frame_ns = 0; zeroed.max_steps = 0;   // as zeroed memory reads it
  k := loop_frame(zeroed, 1.0 / 64.0);
  ok := loop_frame(p.clock, 1.0 / 64.0);
  print("one 1/64 s frame, constructed clock: steps %, clamp % ns\n", ok.steps, ok.clock.max_frame_ns);
  print("one 1/64 s frame, zeroed clock:      steps %, clamp % ns\n", k.steps, k.clock.max_frame_ns);
  s := loop_frame(zeroed, 3.0);
  print("a 3 s stall on the zeroed clock: steps %, overlong %\n", s.steps, s.clock.overlong);
}
```

```shell
coax/bin/coax run aethyr/examples/state-defaults.coax
```

```text
constructed: on_ground true, camera 4.4 m out at phi 1.1071488, pace 1.0, grip 1.0
constructed clock: step 15625000 ns, clamp 250000000 ns, at most 8 steps
one 1/64 s frame, constructed clock: steps 1, clamp 250000000 ns
one 1/64 s frame, zeroed clock:      steps 1, clamp 250000000 ns
a 3 s stall on the zeroed clock: steps 8, overlong 1
```

## The check

`coax/web/state-defaults.test.ts` boots every engine
`apps/openagents-web/build.sh` makes, and the Grid, the way the browser
does: a zeroed state above the module's data, then `coaxial_state_init`. For
each defaulted part it compares the bytes with native `coax_init` of that
type, compiled as its own C program: the oracle is the compiler's native
output, independent of the browser path. A difference names the realm, the
type, the member and the first byte that differs.

A check that compares a value with itself passes for any value, so this one
also proves it is sharp: each engine is booted again without
`coaxial_state_init`, and its zeroed state must differ from native somewhere,
or the realm fails as "a zeroed state passes the comparison". Every defaulted
state has at least one non-zero default, so a realm that skipped its defaults
can't pass.

```shell
apps/openagents-web/build.sh
bun test coax/web/state-defaults.test.ts
```

| What | Result (2026-10-31) |
| --- | --- |
| Realm states matching native `coax_init`, byte for byte | 16 |
| Of those, caught when left zeroed | 16 |
| Run time | 202 s |

The 16 are Belt, Dead Gears, Everglade, Fairy Forest, Garden, Ice Cream Man,
Memorial, Orbit League, Hold the Stair, the villa and the pages with state
defaults; a realm with no declared defaults has nothing to apply. The face's
globals are defaulted by the constructor but not yet compared byte for byte.

## Still to do

Replay (chapter 5) needs more than defaults: a digest of the constructed
state, and of the state after a realm's deterministic setup with a fixed
seed, identical on both hosts. That digest is recorded at the start of every
session record, so it lands with `ec-replay`. The host's `?.` still lets an
engine without the export start zeroed; with every realm built by the same
`build.sh` step that writes it, the realm migration makes the call
unconditional ([chapter 64](ch064-every-realm-runs-on-aethyr.md)).

## Dependencies

[Chapter 1](ch001-a-frame-has-a-fixed-step.md) (the first step runs after
construction). From Coax: struct defaults
([Coax chapter 5](../../coax/book/ch05-structs-enums-unions.md)), memory
([Coax chapter 6](../../coax/book/ch06-memory.md)), targets
([Coax chapter 21](../../coax/book/ch21-targets.md)), and R16 snapshots
([Coax roadmap](../../coax/ROADMAP.md)), which reuse the same construction
when restoring.
