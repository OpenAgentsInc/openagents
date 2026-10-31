# 3. State starts from its defaults

> Engine-core task: `ec-state` (#12609), done in `c5fb95a184`. The browser
> path is [coax/web/defaults.py](../../coax/web/defaults.py), its check is
> [coax/web/state-defaults.test.ts](../../coax/web/state-defaults.test.ts),
> and [aethyr/loop.coax](../loop.coax)'s clock is safe when it was never
> constructed.

On 2026-10-31, Orbit League ran without gravity for about two and a half
hours. Its state declared `spin_g: f64 = 1.0`. The native build applied that
default, but the browser read `0`, because WebAssembly memory starts as
zeros.

This chapter shows how Aethyr constructs a realm's state the same way on
every host before the first step.

## What this chapter teaches

- What a declared default is, and where Coax applies it.
- Why zeroed memory isn't a constructed value.
- How native and browser hosts use the same compiler-generated initializers.
- How to compare initial states byte for byte, and how to confirm that the
  comparison detects a missing default.

## The method

Each field of a realm's state type `T` has either a declared default or the
zero value of its type. Construction depends only on the type:

```jai
init(T) = T.{ f₁ = default₁ or zero(f₁), …, fₙ = defaultₙ or zero(fₙ) }
state   = init(T)          // before any step, on every host
```

In a native build, the declaration `s: [1] T` constructs the state. For
each struct that has a default, the compiler emits
`static T coax_init_t_T(void)` and calls it.

The walking controller's state uses several defaults:

- `on_ground` is `true`.
- The camera starts 4.4 m behind and above the player.
- `pace` and `grip` are `1`.
- The clock from [chapter 1](ch001-a-frame-has-a-fixed-step.md) has its step,
  clamp, and step limit set.

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
  jumps: InputPress;          // jump presses, each taken by the first step after it (aethyr/input.coax)
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

In the browser, the host allocates the state in the module's memory. That
memory starts as zeros, so every default reads as zero: the player starts in
the air, the camera sits on its target, and the clock has no step. To fix
this, the browser host also runs the compiler's initializers:

1. The build runs `coax/web/defaults.py` over the generated C. The script
   exports two functions:
   - `coaxial_state_init(state)` writes every defaulted field of a new state.
   - `coaxial_state_default(i)` returns the location of the `i`-th defaulted
     field. The check uses it.

   A constructor that `_initialize` runs also sets the defaults of the
   module's Coax globals, such as the player and the thumb controls.
2. [`coax/web/host.js`](../../coax/web/host.js) calls `coaxial_state_init` on
   the new state before the first frame.

Both hosts run the same compiler-generated code. No default is copied by
hand.

## Keep the clock running with zeroed fields

A state can still contain zeros in two cases:

- An older host doesn't call `coaxial_state_init`.
- A state is restored from bytes saved before a field existed.

A zeroed clock causes the worst failure. With a clamp of zero, every frame
counts as no time, so the simulation never steps. To prevent this,
`loop_accept_ns` replaces a step, clamp, or step limit that is zero or
negative with its declared default. For the code, see
[the clock listing in chapter 1](ch001-a-frame-has-a-fixed-step.md#the-clock).

The following example,
[`aethyr/examples/state-defaults.coax`](../examples/state-defaults.coax),
prints a constructed player and then runs a zeroed clock:

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

`coax/web/state-defaults.test.ts` starts every engine that
`apps/openagents-web/build.sh` builds, plus the Grid, the same way the
browser does. It allocates a zeroed state and calls `coaxial_state_init`.
Then, for each defaulted field, it compares the bytes with the output of the
native `coax_init` for that type, compiled as a separate C program. If the
bytes differ, the test reports the realm, type, field, and first differing
byte.

The test also confirms that the comparison can fail. It starts each engine a
second time without calling `coaxial_state_init`. The zeroed state must
differ from the native state; if it doesn't, the test fails with "a zeroed
state passes the comparison". Every state with defaults has at least one
non-zero default, so a realm that skips its defaults fails.

```shell
apps/openagents-web/build.sh
bun test coax/web/state-defaults.test.ts
```

| What | Result (2026-10-31) |
| --- | --- |
| Realm states matching native `coax_init`, byte for byte | 16 |
| Of those, caught when left zeroed | 16 |
| Run time | 202 s |

The 16 states come from Belt, Dead Gears, Everglade, Fairy Forest, Garden,
Ice Cream Man, Memorial, Orbit League, Hold the Stair, the villa, and the
pages that declare state defaults. Realms with no declared defaults aren't
included.

The constructor sets the defaults of the module's globals, but the test
doesn't compare them byte for byte yet.

## Still to do

- Replay ([chapter 5](ch005-replay-a-session-exactly.md)) needs a digest of
  the constructed state and of the state after a realm's setup with a fixed
  seed. Both hosts must produce the same digest. Each session record starts
  with this digest, so it lands with `ec-replay`.
- The host calls `coaxial_state_init` with `?.`, so an engine without the
  export still starts with zeros. After every realm is built by the same
  `build.sh` step, the realm migration makes the call unconditional. For
  details, see [chapter 64](ch064-every-realm-runs-on-aethyr.md).

## Dependencies

- [Chapter 1](ch001-a-frame-has-a-fixed-step.md): the first step runs after
  construction.
- From Coax: struct defaults
([Coax chapter 5](../../coax/book/ch05-structs-enums-unions.md)), memory
([Coax chapter 6](../../coax/book/ch06-memory.md)), targets
([Coax chapter 21](../../coax/book/ch21-targets.md)), and R16 snapshots
([Coax roadmap](../../coax/ROADMAP.md)), which reuse the same construction
when restoring.
