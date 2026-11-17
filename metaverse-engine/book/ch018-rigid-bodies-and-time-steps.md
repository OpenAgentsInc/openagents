# 18. Rigid bodies and time steps

> **Status: partly implemented.** [`metaverse-engine/physics/sim/rigid.coax`](../physics/sim/rigid.coax)
> (122 mutants: 119 killed, 3 marked equivalent with their reason) steps a
> batch of free rigid bodies on the CPU in f64 under three named
> profiles: explicit Euler, semi-implicit Euler and an implicit gyroscopic
> step. Its checks measure each profile on free fall, a symmetric top, the
> intermediate-axis flip against the Jacobi elliptic solution, energy and
> momentum, and step halving, and agree with [`metaverse-engine/physics/sim/rigid.coax`](../physics/sim/rigid.coax).
> Device kernels, f32 profiles, sleeping and moving chapter 17's scene onto
> these arrays are not built yet.

A rigid body is the simplest thing an engine moves that can still surprise
you. Its centre of mass obeys Newton, but its rotation obeys Euler's
equations, which couple the three axes: a body spinning about one axis can
start tumbling about another without any torque. A time-stepping method that
handles the first and mishandles the second gains or loses energy, or spins
up on its own. This chapter builds the free-body solver and measures, rather
than assumes, how well each integrator keeps the invariants.

## What this chapter teaches

- The Newton–Euler equations for one body, and quaternion attitude.
- Explicit, semi-implicit and implicit integration, and what each does to
  energy, momentum and stability.
- Why the gyroscopic term needs care, and how an implicit step for it keeps
  a fast-spinning body stable.
- How to state an integrator's error as numbers on invariants, the same way
  Psionic chapter 109 stated its solver's error.

## The physics

For a body of mass `m` with inertia `I_b` in its own frame, attitude
quaternion `q`, world angular velocity `ω` and centre-of-mass velocity `v`:

```jai
m dv/dt          = F
I ω̇ + ω × (I ω)  = τ          // I = R(q) I_b R(q)ᵀ, world frame
dq/dt            = ½ [0, ω] ⊗ q
```

Without forces, kinetic energy `½ m v·v + ½ ω·(I ω)` and angular momentum
`L = I ω` are conserved. Rotation about the largest or smallest principal
axis is stable; about the intermediate axis it is not (the Dzhanibekov
effect), and a correct solver must reproduce that instability, not damp it.

Three steppers, each one line of the same update:

```jai
explicit Euler:      v += Δt F/m;  x += Δt v_old;   ω += Δt I⁻¹(τ − ω × Iω)
semi-implicit Euler: v += Δt F/m;  x += Δt v_new    // symplectic for x, v
implicit gyroscopic: solve f(ω') = I_b(ω' − ω) + Δt ω' × (I_b ω') − Δt τ_b = 0
                     by one Newton step in the body frame
```

Semi-implicit Euler keeps energy bounded for conservative linear motion but
the explicit gyroscopic term injects energy into a spinning body; solving
the gyroscopic term implicitly removes that growth at the cost of slight
dissipation ([Catto, GDC 2015](https://box2d.org/files/ErinCatto_NumericalMethods_GDC2015.pdf)).
The attitude update multiplies by the exponential of the rotation
`exp(½ Δt ω)` and renormalizes `q`, so `|q| = 1` holds to rounding.

## What runs today

[`metaverse-engine/physics/sim/rigid.coax`](../physics/sim/rigid.coax) holds up to 64
bodies as a `Rigid_Set`: arrays of position, attitude, linear velocity,
body-frame angular velocity, mass and inverse mass, principal moments and
their inverses, and a world-frame force and torque that each step clears.
`rigid_set_step` advances every body by one profile with no branch on body
kind. A static body has mass, inverse mass and inverse inertia zero, so
gravity, force and torque leave it where it is. The implicit gyroscopic
profile takes one Newton step on
`f(ω') = (ω' − ω) + Δt I⁻¹(ω' × Iω' − τ)`; with `I⁻¹` on the left, a static
body's Jacobian is the identity and its step is zero.

The example [`spin-rigid-bodies.coax`](../../metaverse-engine/examples/spin-rigid-bodies.coax)
runs each profile on the chapter's cases:

<!-- source: metaverse-engine/examples/spin-rigid-bodies.coax example -->
```jai
// A body thrown at (3, 4) m/s from 100 m, after 2 s at dt 0.01.
throw :: (p: Rigid_Profile) -> f64 {
  s: [1] Rigid_Set; i := rigid_add(s[..], 2.0, d3(1.0, 1.0, 1.0), d3(0.0, 100.0, 0.0), DQ.{});
  s[0].vel[i] = d3(3.0, 4.0, 0.0);
  for k: 0..<200 { rigid_set_step(s[..], p, 0.01, d3(0.0, -9.81, 0.0)); }
  return s[0].pos[i].y - (100.0 + 4.0 * 2.0 - 0.5 * 9.81 * 4.0);
}

// The time between the first two sign changes of w2, and the energy and |L|
// drift after `steps` steps, for a box spun about its intermediate axis.
Flip :: struct { interval: f64; energy: f64; spin: f64; }
flip :: (p: Rigid_Profile, dt: f64, steps: int) -> Flip {
  out: Flip; w0 := d3(0.0, 1.0, 0.05);
  s: [1] Rigid_Set; i := rigid_add(s[..], 1.0, d3(1.0, 2.0, 3.0), D3_ZERO, DQ.{}); s[0].omega[i] = w0;
  e0 := rigid_energy(s[0], i); l0 := d3_len(rigid_spin(s[0], i));
  c: [2] f64; n := 0; last := w0.y;
  for k: 1..<steps + 1 {
    rigid_set_step(s[..], p, dt, D3_ZERO);
    now := s[0].omega[i].y;
    if n < 2 && (now < 0.0) != (last < 0.0) { c[n] = cast(f64) k * dt - dt * now / (now - last); n += 1; }
    last = now;
  }
  out.interval = c[1] - c[0];
  out.energy = rigid_energy(s[0], i) / e0 - 1.0; out.spin = d3_len(rigid_spin(s[0], i)) / l0 - 1.0;
  return out;
}

main :: () {
  print("ballistic throw, 2 s at dt 0.01, height minus x0 + v0 t + g t^2 / 2 (g dt t / 2 = %):\n", 0.5 * 9.81 * 0.01 * 2.0);
  for p: PROFILES { print("  %: % m\n", rigid_profile_name(p), sig(throw(p))); }

  print("symmetric top I = (1, 1, 2), w3 = 1 rad/s, 1 s at dt 0.001: precession angle (want 1 rad)\n");
  for p: PROFILES {
    s: [1] Rigid_Set; i := rigid_add(s[..], 1.0, d3(1.0, 1.0, 2.0), D3_ZERO, DQ.{}); s[0].omega[i] = d3(0.1, 0.0, 1.0);
    for k: 0..<1000 { rigid_set_step(s[..], p, 0.001, D3_ZERO); }
    w := s[0].omega[i];
    print("  %: % rad\n", rigid_profile_name(p), sig(acos_d(w.x / sqrt(w.x * w.x + w.y * w.y))));
  }

  want := rigid_flip_interval(d3(1.0, 2.0, 3.0), d3(0.0, 1.0, 0.05));
  print("box I = (1, 2, 3) spun at w = (0, 1, 0.05): w2 flips every 2 K / lambda = % s\n", sig(want));
  print("  10^5 steps at dt 0.001 (100 s):\n");
  for p: PROFILES {
    f := flip(p, 0.001, 100000);
    print("  %: flip % s (% off), energy %, |L| %\n", rigid_profile_name(p), sig(f.interval), sig(f.interval - want), sig(f.energy), sig(f.spin));
  }
}
```

Run it with:

```shell
coax/bin/coax run metaverse-engine/examples/spin-rigid-bodies.coax
```

```text
ballistic throw, 2 s at dt 0.01, height minus x0 + v0 t + g t^2 / 2 (g dt t / 2 = 0.0981):
  rigid-explicit-f64-v1: 0.0981 m
  rigid-semi-implicit-f64-v1: -0.0981 m
  rigid-implicit-gyro-f64-v1: -0.0981 m
symmetric top I = (1, 1, 2), w3 = 1 rad/s, 1 s at dt 0.001: precession angle (want 1 rad)
  rigid-explicit-f64-v1: 0.999999667 rad
  rigid-semi-implicit-f64-v1: 0.999999667 rad
  rigid-implicit-gyro-f64-v1: 0.999999667 rad
box I = (1, 2, 3) spun at w = (0, 1, 0.05): w2 flips every 2 K / lambda = 13.258606867 s
  10^5 steps at dt 0.001 (100 s):
  rigid-explicit-f64-v1: flip 13.258607508 s (6.41e-07 off), energy 0.008916824, |L| 0.004417573
  rigid-semi-implicit-f64-v1: flip 13.258607508 s (6.41e-07 off), energy 0.008916824, |L| 0.004417573
  rigid-implicit-gyro-f64-v1: flip 13.258606937 s (7e-08 off), energy -0.008798216, |L| -0.004377387
```

The throw lands exactly where the closed form says each integrator should:
semi-implicit Euler advances position with the new velocity and lands
`g Δt t / 2` low, explicit Euler with the old velocity and lands that much
high. Without torque, explicit and semi-implicit Euler update `ω` the same
way and differ only in which `ω` turns the attitude, so their spin
measurements match. Both gain energy: 0.9% over 100 s on the flipping box,
and 6% in one second for a box spun at 20 rad/s at `Δt = 0.01`. The
implicit step loses about as much on the box instead, and only 0.12% on the
fast spin. Every profile reproduces the intermediate-axis flip rather than
damping it; the implicit step's flip interval is 7×10⁻⁸ s from
`2K(k)/λ`.

The checks run with:

```shell
coax/bin/coax test metaverse-engine/physics/sim/rigid.coax --library
```

They pin free fall to `x₀ + v₀t + ½gt² ± ½gΔt t` per profile, keep static
bodies still, match momentum changes to impulses, measure precession and the
flip interval, show explicit energy growth against implicit dissipation,
halve the energy error by halving the step (all three profiles are first
order), and match `metaverse-engine/physics/sim/rigid.coax`'s RK4 attitude to 10⁻⁶ after 1,000
steps.

## The design in the Metaverse Engine

The solver owns arrays (structure of arrays, chapter 17): position,
attitude, linear and angular velocity, inverse mass, body-frame inverse
inertia, and an accumulated force and torque per body. One kernel advances
every body, with no branches on body kind; static bodies have inverse mass
zero. Each integrator is a named profile (`rigid-semi-implicit-f64-v1`,
`rigid-implicit-gyro-f32-v1` and so on), so a recording says which one ran.

The OpenAgents Metaverse Engine's module is the independent reference for the first profile. It uses
RK4 for torque-free attitude and keeps a momentum ledger against named
external terms; the Metaverse Engine kernel is a separate implementation checked
against it, not a copy. Sleeping (a body at rest for a while is skipped) is
kept from the Metaverse Engine but is a profile option, because it changes results and is
incompatible with differentiation (chapter 32).

f64 is the reference precision. Apple GPUs have no f64; the device profile
uses f32 with compensated sums where measurement shows they are needed, and
reports its own error budget rather than inheriting the CPU's.

## Bodies that touch

[`metaverse-engine/physics/sim/bodies.coax`](../physics/sim/bodies.coax)
(2,095 mutants on its first pass, 2026-11-17: 1,814 killed, 281 survive, 272 of them in the module and 9 in its checks' helpers; they are not yet killed or marked, most of them in `bd_snapshot_read`, `bd_rest`, `bd_keep`, `bd_swap`, `bd_collide`, `bd_prepare` and `bd_step`, so the engine's zero-survivor standard is not met for this module yet) is the first solver here that holds a world of things
together: boxes and balls on a ground plane that fall, land, stack, tip,
roll and sleep, pushed by characters, at a fixed step, in a form a second
machine can repeat. It composes this chapter's store and its implicit
gyroscopic step with chapter 20's sweep and prune and chapter 21's box
manifolds, and adds the rest. GameBuilder's web world is its first
consumer (its blocks and its ball).

| Piece | What it does |
| --- | --- |
| Order | Every body has an id from its owner, and bodies are kept in id order. The broad phase, the contacts, the solver and the islands run in that order, whatever order bodies were added in and whatever was removed between. |
| Contacts | The four lowest corners of a box against the ground, clipped face manifolds between boxes (each grown by half the reach, so resting faces always meet in a whole manifold), a ball against a box or a ball. A point that is still short of touching may close its gap in the step and no more; the bounce it is owed is paid the next step. Nothing passes through the ground. |
| Solver | Sequential impulses, 10 passes, warm started from the last step's impulses matched by feature: a normal impulse that only pushes, Coulomb friction on two tangents, restitution above 1 m/s. Every target is read before any warm start is applied. |
| Overlap | Pushed out by a second solve on pseudo-velocities (split impulses), 0.2 of the overlap past a 5 mm slop a step, 3 m/s at most. The push never becomes speed, so a body put inside another parts from it and stops. |
| Sleep | Islands (bodies joined by contacts) sleep together after half a second of stillness and wake together when something moving touches one. A sleeper is where it was to the bit. |
| Movers | Kinematic upright capsules, each with an id, taken in id order: the characters. A mover drives what it walks into toward its own speed with a capped force, `bd_push_share`: all of the force on a standing body, none on one already going at the mover's shove speed, and between them `(1 - v) / (1 + curve v)`. Bodies never move a mover, and it does not push what is low enough to step onto. Two movers on one block are two capped constraints in one solve: the stronger wins, equal ones hold it. |
| Rolling | A ball's spin is opposed by a torque of its `roll` length times its load. |
| Time | `bd_advance` takes seconds and makes whole steps of 1/60 s from an accumulator, four a call at most; `bd_pose` blends the last two steps for drawing. |
| Shared and local | A body is shared (consequential: what a server replicates) unless marked local (decoration). A local body is moved by shared ones and never moves, wakes or keeps awake a shared one, and has its own share of the contact room: the shared bodies' state is the same bits with any local bodies or none. |
| Snapshot | `bd_snapshot_write` and `bd_snapshot_read`: a 32-byte header (`BDS1`, version 2, record size, step, count, the scene's gravity) and a 112-byte record a shared body (id, sleep group, position, attitude, velocity, body-frame spin, as little-endian f64), in id order. Reading clears what a step keeps between steps, so every scene that reads one snapshot goes on bit for bit the same. |
| Gravity | `bd_gravity`: the scene's gravity is a setting a game may change as it runs. A change wakes every body. It is in the digest, the snapshot's header and each stored step of the history, so a run with a change in it repeats bit for bit and a rewind puts it back (gamebuilder.org's world settings: a slider from a tenth to five times the normal) |
| History | `Bd_History` keeps the last 64 steps of every body (`bd_at`, `bd_between`), and for the newest 16 the contact impulses too: `bd_rewind` to one of those and stepping again with the same movers repeats the first run bit for bit. |

What it measures, in its checks: a box dropped 4 m lands without entering
the ground, rebounds to its restitution squared, and sleeps within two
seconds; a stack of three stays to the bit while it sleeps; a tall box
tilted short of its balance angle rights itself and past it falls; friction
stops a slide in `v^2 / 2 mu g`; a mover shoves a cube at the speed where
its force curve meets the cube's friction; 48 boxes and a ball dropped in a
heap are all asleep within ten seconds, never deeper than 4 cm in the
ground at any step; a recorded run of 600 steps (two movers, bodies added,
placed and removed) replays into a fresh scene with the same state hash at
every step, whatever order the bodies were first added in and however the
seconds were cut into frames.

What is promised across machines: a step uses only IEEE add, subtract,
multiply, divide and square root, and turns a body with the engine's strict
sine and cosine (`bd_exp`, on `core/math.coax`), never the host's. So two
copies of one build agree, and a native room and a wasm page are expected
to. That last has not yet been run for this module: until it has, a client
treats a server's snapshot as the truth and its own stepping as a
prediction. For shared worlds the next steps are snapshot interpolation
(pages draw shared bodies from snapshots a little in the past, with
`bd_between`), then prediction of the whole island a player's mover touches
(`bd_rewind` and re-step when a snapshot disagrees); see
[`docs/research/2026-11-17-shared-physics-multiplayer-plan.md`](../../docs/research/2026-11-17-shared-physics-multiplayer-plan.md).

Not built: joints, hulls and capsules as bodies, continuous collision
between two fast bodies, more than 64 bodies, device kernels, the network.
The Physics Lab (chapter 37) still measures the older kernel composition
against Rapier and Jolt, not this module.

## What it still builds

- Generated CUDA and Metal kernels for `rigid_set_step` through Psionic
  chapter 37's compute subset, checked against the CPU profiles.
- f32 device profiles with compensated sums where measurement shows they
  are needed, each with its own error budget.
- This chapter's batched profiles have no sleeping; `bodies.coax` (above) has islands that sleep.
- A thing set down at rest and not dropped is `bd_still`: the body sleeps where it is, in a sleep group of its own, and stays exactly there (beside and on top of others, checked over ten seconds of steps) until a mover, a moving body, a change of gravity or an impulse wakes it. With no gravity (`bd_gravity(b, 0)`) nothing falls: what rests stays and sleeps, what is in the air hangs, an impulse still moves a body; nothing in a step divides by gravity.
- Chapter 17's scene stepping its rigid solver through these arrays instead
  of its falling centre of mass.

## Acceptance checks and oracles

- Free fall and a ballistic arc against the closed form `x(t) = x₀ + v₀t + ½gt²`
  (exact for semi-implicit Euler up to its known `O(Δt)` offset, which the
  check states).
- Torque-free symmetric top: precession rate `Ω = (I₃ − I₁)ω₃ / I₁` in the
  body frame against the analytic value.
- Intermediate-axis spin: the flip period against the Jacobi elliptic
  solution, and `|L|` and energy drift reported per profile over 10⁵ steps.
- Momentum ledger: total momentum changes only by named external impulses.
- Agreement with `metaverse-engine/physics/sim/rigid.coax` on shared cases within a stated
  tolerance; device kernels against the CPU profile within theirs.
- Step-halving: error falls at the integrator's stated order.

## How Genesis does it

Genesis treats free bodies as articulations with a free joint in its rigid
solver and integrates them with its constraint-aware schemes; its default is
`approximate_implicitfast`, which saves work relative to the fuller implicit
path while approximating terms that matter for stiff systems
([constraints](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/rigid_solver/constraints.html),
[solver options](https://genesis-world.readthedocs.io/en/v1.4.5/_modules/genesis/options/solvers.html)).
MuJoCo offers the same family (Euler, `implicitfast`, implicit, RK4)
([MuJoCo computation](https://mujoco.readthedocs.io/en/stable/computation/index.html)).
We expose the same trade-off as named profiles, with each profile's
invariant drift measured and published; Genesis states the trade-off in
prose and leaves measurement to the user.

## Dependencies

Chapter 17's scene, Psionic chapter 4's numerics and Psionic chapter 37's compute subset for
devices. From Coax: units and frames (R12) for poses and velocities, strict
math (R15) for the reproducible profiles. Chapter 19 later generalizes the
free-body integrator to joints; it is not needed to construct or qualify
this chapter's free-body profile.
