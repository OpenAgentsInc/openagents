# 166. Rigid bodies and time steps

> **Status: unimplemented — design draft.** Verse already steps free rigid
> bodies in f64 ([`verse/rigid.coax`](../../verse/rigid.coax)); this chapter
> plans the Psionic solver that generalizes it to batches, devices and
> stated precision profiles. Nothing here runs yet.

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
  chapter 109 stated its solver's error.

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

## The design in Psionic

The solver owns arrays (structure of arrays, chapter 165): position,
attitude, linear and angular velocity, inverse mass, body-frame inverse
inertia, and an accumulated force and torque per body. One kernel advances
every body, with no branches on body kind; static bodies have inverse mass
zero. Each integrator is a named profile (`rigid-semi-implicit-f64-v1`,
`rigid-implicit-gyro-f32-v1` and so on), so a recording says which one ran.

Verse's module is the independent reference for the first profile. It uses
RK4 for torque-free attitude and keeps a momentum ledger against named
external terms; the Psionic kernel is a separate implementation checked
against it, not a copy. Sleeping (a body at rest for a while is skipped) is
kept from Verse but is a profile option, because it changes results and is
incompatible with differentiation (chapter 180).

f64 is the reference precision. Apple GPUs have no f64; the device profile
uses f32 with compensated sums where measurement shows they are needed, and
reports its own error budget rather than inheriting the CPU's.

## What it builds

- `psionic/lib/sim/rigid.coax`: body arrays, force accumulation and the
  three integrator profiles, CPU first, then generated CUDA and Metal
  kernels through chapter 37's compute subset.
- An example: a symmetric top, a box flipped about its intermediate axis and
  a ballistic throw, each printing its invariants every 100 steps.

## Acceptance checks and oracles

- Free fall and a ballistic arc against the closed form `x(t) = x₀ + v₀t + ½gt²`
  (exact for semi-implicit Euler up to its known `O(Δt)` offset, which the
  check states).
- Torque-free symmetric top: precession rate `Ω = (I₃ − I₁)ω₃ / I₁` in the
  body frame against the analytic value.
- Intermediate-axis spin: the flip period against the Jacobi elliptic
  solution, and `|L|` and energy drift reported per profile over 10⁵ steps.
- Momentum ledger: total momentum changes only by named external impulses.
- Agreement with `verse/rigid.coax` on shared cases within a stated
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

Chapter 165's scene, chapter 4's numerics and chapter 37's compute subset for
devices. From Coax: units and frames (R12) for poses and velocities, strict
math (R15) for the reproducible profiles. Chapter 167 generalizes the free
body to joints.
