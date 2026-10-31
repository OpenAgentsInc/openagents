# 165. A scene owns the state

> **Status: unimplemented — design draft.** No code in this chapter runs
> yet. It fixes what Part XVIII's engine is made of before any solver is
> written; the [parity plan](../docs/genesis-parity.md) explains why the
> part exists.

Every chapter so far has owned one kind of state: a tensor, a model's cache,
a decision's prefix. A physics engine owns many at once. A robot arm is a
tree of rigid links, the block it lifts may be an elastic mesh, the table is
static geometry, and the water in the cup is particles. Each is advanced by a
different solver, yet one step must advance all of them together, in a
fixed order, with forces passing between them. This chapter designs the
container that makes that possible: the scene.

## What this chapter teaches

- How to describe a world as data and then *build* it into solver arrays,
  so that stepping never allocates, looks anything up by name or changes
  shape.
- Why geometry, physical material and appearance are three separate
  descriptions of one entity.
- How a step is scheduled across several solvers and a coupler, and why the
  schedule is part of the result.
- How a scene refuses a combination it cannot simulate before it allocates
  anything.

## The model

A scene is a state vector and a transition. Collect every solver's state
(rigid joint positions and velocities, particle positions and velocities,
mesh vertices, grid values) into `x`, the controls into `u` and the fixed
parameters (masses, stiffnesses, friction, time step) into `θ`. One step is

```jai
x[k+1] = Φ(x[k], u[k]; θ, Δt)
Φ      = finalize ∘ (exchange ∘ advance_n ∘ ... ∘ advance_1 ∘ prepare)^s
```

where `s` is the number of substeps (each of length `Δt / s`), `advance_i`
is solver `i`'s own update, and `prepare`/`exchange` belong to the coupler
(chapters 177 and 178). Everything later in this part is a choice of `Φ`'s
pieces. Because floating-point addition is not associative, the *order* of
those pieces is part of the definition: two schedules that are equal in real
arithmetic give different bits, so the schedule is recorded with the scene.

## The design in Psionic

**Three descriptions per entity.** A *shape* gives geometry and initial pose
(a primitive, a mesh, an imported robot from chapter 183). A *material*
chooses the solver and its constitutive parameters (rigid with a density;
elastic with Young's modulus and Poisson ratio; sand with a friction angle).
An *appearance* says how it is drawn, and nothing else. A change of
appearance can never alter a trajectory: drawing code reads state under an
effect that cannot write it (Coax R11, R20's presentation profile).

**Describe, build, step.** A `SceneSpec` is an ordinary checked Coax value:
bounded arrays of entities, solver options, coupler choice, gravity, time
step and substeps, and a batch size (chapter 179). `scene_build` admits the
spec and lays out each solver's state as its own structure of arrays: all
rigid bodies' poses together, all particles' positions together. An entity
becomes a handle, `(solver, first, count)`, into those arrays, so reading a
robot's joint angles is a slice, not a search. Building is where shapes are
sampled into particles, meshes tetrahedralized and collision geometry
prepared; stepping does none of that.

**Admission before allocation.** The engine publishes a compatibility
matrix as data: for each solver, backend, coupler, gradient mode (chapter
180), renderer (chapter 182) and precision, whether the combination is
supported, and if not, why. `scene_build` checks the spec against it and
returns a typed refusal that names the first unsupported pair, such as
"SPH with gradients: no backward pass" or "IPC coupling on Metal: needs f64".
Admission is policy, so this file is `#authority` and its mutation run must
leave zero survivors.

**One schedule, recorded.** The step is a fixed sequence: apply controls,
then per substep the coupler's prepare, each active solver's advance in a
fixed solver order, the coupler's exchange, then finalize and sensors. The
sequence and the scene's digest are written into every recording (chapter
184), so a replay can say not only that the result differs but at which
stage of which substep.

**Topology is fixed between builds.** Adding or removing bodies requires a
rebuild in the first profile. A streaming world (Verse adds and removes
objects constantly) needs pooled capacity and handle generations; that is a
later profile with its own checks, and the scene says which profile it was
built under.

## What it builds

- `psionic/lib/sim/scene.coax`: `SceneSpec`, entity handles, `scene_build`,
  `scene_step`, `scene_reset` and slice accessors.
- `psionic/lib/sim/admit.coax` (`#authority`): the compatibility matrix and
  build-time refusals.
- An example that builds a plane, a box and a sphere, steps them, and prints
  a state digest after each step.

## Acceptance checks and oracles

- The same spec builds the same arrays, byte for byte, and the same digest
  sequence over 1,000 steps on the same target.
- `scene_reset` to a snapshot, then stepping, reproduces the original run
  bit for bit.
- Every slice accessor round-trips: writing through a handle and reading the
  whole array changes exactly that slice.
- Every unsupported matrix cell has a fixture that is refused with its named
  reason, and no supported cell is refused; mutation leaves no survivors.
- A change only to appearance leaves the state digest unchanged.

## How Genesis does it

Genesis organizes everything around a `Scene`, with entities made of a
*Morph* (geometry and pose), a *Material* (physics and solver choice) and a
*Surface* (appearance), and solver arrays that entity methods address by
slice ([concepts](https://genesis-world.readthedocs.io/en/latest/user_guide/configuration/concepts.html)).
A coupled substep prepares coupling, advances solvers, exchanges momentum and
finalizes ([coupling](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/coupling/index.html),
[simulator.py](https://github.com/Genesis-Embodied-AI/genesis-world/blob/v1.4.5/genesis/engine/simulator.py)).
We adopt the same separation and the same build/step lifecycle. We differ in
three ways: the scene is described in Coax rather than Python; the schedule
and its digest are part of the recorded result; and unsupported combinations
are refused from a published matrix at build time rather than discovered
during a run.

## Dependencies

Tensor views and placement (chapters 3 and 6), the tensor IR's plans
(chapter 38), and the counter-based RNG for any sampling during build. From
Coax: stable identities (R5), replay inventories (R16), provenance-typed
parameters (R20) and, for the batch dimension and layouts, R21.
