# 3. State starts from its defaults

> Engine-core task: `ec-state` (#12609), done in `c5fb95a184`. Since
> `ax-state-construction` (#12611), the compiler writes the browser path:
> each library header declares `coax_default_T`, and each page's C face
> calls it. The check is
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
this, the browser host runs the compiler's initializers too:

1. For each type in a library's header that has a declared default, the
   compiler defines and declares `void coax_default_T(T *p)`. It writes
   zeros and then every default into `*p`, in place, so a state larger than
   the stack is safe.
2. Each page's C face exports `coaxial_state_init(state)`, which calls
   `coax_default_T` on a new state. A constructor that `_initialize` runs
   calls it for the face's own globals, such as the player and the thumb
   controls.
3. [`coax/web/host.js`](../../coax/web/host.js) calls `coaxial_state_init` on
   the new state before the first frame.

Both hosts run code the compiler generates from the same declarations. No
default is copied by hand, and no build step edits the generated C.

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
Then, for each defaulted state, it compares the bytes with the output of the
native `coax_init` for that type, compiled as a separate C program. If the
bytes differ, the test reports the realm, type, field, and first differing
byte.

It also builds each face natively with its units, so the face's
constructors run, and compares every face global of a defaulted type, each
element of an array, with that type's `coax_default_T`.
`coax/tests/state_defaults.py` checks that `coax_default_T` writes the bytes
native `coax_init` gives.

The test also confirms that the comparison can fail. It starts each engine a
second time without calling `coaxial_state_init`. The zeroed state must
differ from the native state; if it doesn't, the test fails with "a zeroed
state passes the comparison". Every defaulted type must have a non-zero
default, so a realm or a global that skips its defaults fails.

```shell
apps/openagents-web/build.sh
bun test coax/web/state-defaults.test.ts
```

| What | Result (2026-10-31) |
| --- | --- |
| Realm states matching native `coax_init`, byte for byte | 17 |
| Of those, caught when left zeroed | 17 |
| Face globals matching their type's defaults | 20 |
| Run time | 68 s |

The 17 states come from Belt, Dead Gears, Everglade, Fairy Forest, Garden,
Ice Cream Man, Memorial, Orbit League, Hold the Stair, the villa, Water Lab,
and the pages that declare state defaults. Realms with no declared defaults
aren't included. The 20 globals are the player and thumbs of each walking
realm, the Grid's session, the haunted house's entities, people and house, and the homepage
reader's query.

## Still to do

- Each session record starts with the digest of the constructed state, and
  `replay_play` refuses a replay from any other state
  ([chapter 5](ch005-replay-a-session-exactly.md)). Each realm writes its
  own state digest as it moves onto replay, starting with the Grid.
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
