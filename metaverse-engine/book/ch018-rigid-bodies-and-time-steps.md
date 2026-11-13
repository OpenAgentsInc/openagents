# 18. Rigid bodies and time steps

> **Status: partly implemented.** [`metaverse-engine/physics/sim/rigid.coax`](../physics/sim/rigid.coax)
> (122 mutants: 119 killed, 3 marked equivalent with their reason) steps a
> batch of free rigid bodies on the CPU in f64 under three named
> profiles: explicit Euler, semi-implicit Euler and an implicit gyroscopic
> step. Its checks measure each profile on free fall, a symmetric top, the
> intermediate-axis flip against the Jacobi elliptic solution, energy and
> momentum, and step halving, and agree with [`metaverse-engine/physics/rigid.coax`](../physics/rigid.coax).
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
order), and match `metaverse-engine/physics/rigid.coax`'s RK4 attitude to 10⁻⁶ after 1,000
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

## What it still builds

- Generated CUDA and Metal kernels for `rigid_set_step` through Psionic
  chapter 37's compute subset, checked against the CPU profiles.
- f32 device profiles with compensated sums where measurement shows they
  are needed, each with its own error budget.
- Sleeping as a profile option.
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
- Agreement with `metaverse-engine/physics/rigid.coax` on shared cases within a stated
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
