# 17. A scene owns the state

> **Status: partly implemented.** [`metaverse-engine/physics/sim/scene.coax`](../physics/sim/scene.coax)
> (`#authority`, 207 mutants, all killed) runs the scene's container on the
> CPU in f64: a `Scene_Spec` of shapes, materials and appearances, admission
> against a published compatibility matrix, a build into per-solver arrays
> addressed by handle, a fixed recorded schedule, state digests and reset by
> value copy. Its two solvers are deliberately small (a falling centre of
> mass and falling sample points, both stopped by the ground plane);
> [chapter 18](ch018-rigid-bodies-and-time-steps.md) replaces them. Tensor
> views and placement, a coupler, batch size, sampling meshes and writing
> the schedule into recordings are not built yet.

Every chapter so far has owned one kind of state: a clock, a figure, a
camera, a realm's colliders. A physics engine owns many at once. A robot arm is a
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
(chapters 29 and 30). Everything later in this part is a choice of `Φ`'s
pieces. Because floating-point addition is not associative, the *order* of
those pieces is part of the definition: two schedules that are equal in real
arithmetic give different bits, so the schedule is recorded with the scene.

## The design in the Metaverse Engine

**Three descriptions per entity.** A *shape* gives geometry and initial pose
(a primitive, a mesh, an imported robot from chapter 35). A *material*
chooses the solver and its constitutive parameters (rigid with a density;
elastic with Young's modulus and Poisson ratio; sand with a friction angle).
An *appearance* says how it is drawn, and nothing else. A change of
appearance can never alter a trajectory: drawing code reads state under an
effect that cannot write it (Coax R11, R20's presentation profile).

**Describe, build, step.** A `SceneSpec` is an ordinary checked Coax value:
bounded arrays of entities, solver options, coupler choice, gravity, time
step and substeps, and a batch size (chapter 31). `scene_build` admits the
spec and lays out each solver's state as its own structure of arrays: all
rigid bodies' poses together, all particles' positions together. An entity
becomes a handle, `(solver, first, count)`, into those arrays, so reading a
robot's joint angles is a slice, not a search. Building is where shapes are
sampled into particles, meshes tetrahedralized and collision geometry
prepared; stepping does none of that.

**Admission before allocation.** The engine publishes a compatibility
matrix as data: for each solver, backend, coupler, gradient mode (chapter
32), renderer (chapter 34) and precision, whether the combination is
supported, and if not, why. `scene_build` checks the spec against it and
returns a typed refusal that names the first unsupported pair, such as
"SPH with gradients: no backward pass" or "IPC coupling on Metal: needs f64".
Admission is policy, so this file is `#authority` and its mutation run must
leave zero survivors.

**One schedule, recorded.** The step is a fixed sequence: apply controls,
then per substep the coupler's prepare, each active solver's advance in a
fixed solver order, the coupler's exchange, then finalize and sensors. The
sequence and the scene's digest are written into every recording (chapter
36), so a replay can say not only that the result differs but at which
stage of which substep.

**Topology is fixed between builds.** Adding or removing bodies requires a
rebuild in the first profile. A streaming world (a Metaverse Engine realm adds and
removes objects constantly) needs pooled capacity and handle generations; that is a
later profile with its own checks, and the scene says which profile it was
built under.

## What runs today

The program
[`metaverse-engine/examples/scene-three-shapes.coax`](../../metaverse-engine/examples/scene-three-shapes.coax)
asks for two scenes the engine can't run, then builds a plane, a box and a
sphere of particles, steps them, and prints a state digest after each step.
Last, it repaints both shapes and runs the original and the repainted scene
for 1,000 steps.

<!-- source: metaverse-engine/examples/scene-three-shapes.coax example -->
```jai
#load "../physics/sim/scene.coax";
#import "Basic";

// A plane, a 1 m box of density 500 at 2 m, and a 0.5 m sphere of particles at 4 m.
three_shapes :: () -> Scene_Spec {
  spec: Scene_Spec;
  spec.count = 3;
  spec.entities[0] = Entity.{shape = Shape.{kind = Shape_Kind.plane}, material = Material.{solver = Solver.ground}};
  spec.entities[1] = Entity.{shape = Shape.{kind = Shape_Kind.box, size = SV3.{x = 1, y = 1, z = 1}, at = SV3.{y = 2}},
                             material = Material.{solver = Solver.rigid, density = 500}, appearance = Appearance.{rgba = 0xd04020ff}};
  spec.entities[2] = Entity.{shape = Shape.{kind = Shape_Kind.sphere, size = SV3.{x = 0.5}, at = SV3.{x = 3, y = 4}},
                             material = Material.{solver = Solver.particles}, appearance = Appearance.{rgba = 0x2060d0ff}};
  return spec;
}

main :: () {
  // Admission names the first unsupported pair before anything is built.
  on_gpu := three_shapes();
  on_gpu.backend = Backend.cuda;
  learn := three_shapes();
  learn.gradients = true;
  print("cuda: %, gradients: %\n", scene_admit(on_gpu), scene_admit(learn));

  s: [1] Scene;
  print("build: %\n", scene_build(three_shapes(), s[..]));
  box := s[0].handles[1];
  ball := s[0].handles[2];
  print("box: % [%, %), ball: % [%, %)\n", box.solver, box.first, box.first + box.count,
        ball.solver, ball.first, ball.first + ball.count);
  for step: 1..<4 {
    scene_step(s[..]);
    print("step %: box y %, ball's lowest point y %, digest %\n", step,
          scene_body_y(s[0], box, 0), scene_body_y(s[0], ball, 0), scene_digest(s[0]));
  }

  // Repaint everything and run both to 1,000 steps: the digests agree.
  painted := three_shapes();
  painted.entities[1].appearance.rgba = 0xffffffff;
  painted.entities[2].appearance.rgba = 0x000000ff;
  a: [1] Scene;
  b: [1] Scene;
  built_a := scene_build(three_shapes(), a[..]);
  built_b := scene_build(painted, b[..]);
  print("built: %, %\n", built_a, built_b);
  for i: 0..<1000 { scene_step(a[..]); scene_step(b[..]); }
  print("1000 steps: box y %, digest %, repainted digest %\n", scene_body_y(a[0], box, 0),
        scene_digest(a[0]), scene_digest(b[0]));
}
```

```shell
$ coax/bin/coax run metaverse-engine/examples/scene-three-shapes.coax
cuda: backend_not_built, gradients: no_backward_pass
build: none
box: rigid [0, 1), ball: particles [0, 8)
step 1: box y 1.999019, ball's lowest point y 3.749019, digest 13588535490896844969
step 2: box y 1.997057, ball's lowest point y 3.7470570000000003, digest 16207048614719290469
step 3: box y 1.9941140000000002, ball's lowest point y 3.744114, digest 17612693945783445760
built: none, none
1000 steps: box y 0.5, digest 16248280601224408613, repainted digest 16248280601224408613
```

Each line maps to a piece of the design above:

- **Admission before allocation.** `scene_admit` checks every entity against
  `MATRIX`, one row per solver and backend. Only the CPU rows are built, a
  gradient run needs a backward pass that no solver has, and single precision
  is refused because ground contact compares f64 heights. A refused build
  writes nothing: `scene_build` admits first and fills the caller's scene
  only after every entity passes.
- **Handles.** The box is the rigid solver's values `[0, 1)` and the sphere is
  the particle solver's `[0, 8)`: its bounding box sampled at eight points.
  `scene_body_y` and `scene_push` read and write through a handle; the
  library's checks confirm that a write through one handle changes exactly
  its slice.
- **One schedule.** `scene_schedule` lists one step's stages: controls, then
  per substep the coupler's prepare, the rigid solver, the particle solver
  and the coupler's exchange, then finalize. The prepare and exchange stages
  do nothing until a coupler exists.
- **Digests.** `scene_digest` is FNV-1a over the step count and the bits of
  every live state value. Appearance isn't state, so the repainted scene has
  the same digest after 1,000 steps. The checks also step two builds of one
  spec 1,000 times with equal digests, copy the scene at step 37 as a
  snapshot, and replay from it to the same 963 digests.

## What it builds

- Built: [`metaverse-engine/physics/sim/scene.coax`](../physics/sim/scene.coax) holds
  `Scene_Spec`, entity handles, `scene_build`, `scene_step`, the slice
  accessors, the compatibility matrix and the build-time refusals in one
  `#authority` file. Reset is a value copy of `Scene`, which holds no views.
- Built: [`metaverse-engine/examples/scene-three-shapes.coax`](../../metaverse-engine/examples/scene-three-shapes.coax)
  builds a plane, a box and a sphere, steps them, and prints a digest after
  each step.
- Not yet: solver arrays as Psionic tensor views with a placement, a coupler,
  a batch size, mesh and robot shapes, sampling with the counter-based RNG,
  and the schedule and digest written into recordings (chapter 36).

## Acceptance checks and oracles

These run as the library's checks on every build that loads it:

- The same spec builds the same arrays and the same digest sequence over
  1,000 steps.
- Copying the scene at step 37 and stepping from the copy reproduces the
  original digests bit for bit.
- Writing through a handle changes exactly that slice, including handles
  whose slices start past the first entity's.
- Every cell of the matrix is checked: each CPU row is built, and every
  other backend, a gradient run and single precision are refused with their
  named reason. Each spec refusal has a fixture, and mutation leaves no
  survivors.
- A change only to appearance leaves the digest unchanged, and a change to
  any one state value changes it.

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

Tensor views and placement (Psionic chapters 3 and 6), the tensor IR's plans
(Psionic chapter 38), and the counter-based RNG for any sampling during build. From
Coax: stable identities (R5), replay inventories (R16), provenance-typed
parameters (R20) and, for the batch dimension and layouts, R21.

## Declared-state adoption (#12991)

A scene can publish declared facts together with the six resource roots its typed predicates select. Preparation owns references privately; stale or missing members preserve the complete previous world until a paid-step publication succeeds. See [the runtime guide](../state/README.md) and [qualification](../../docs/verification/declared-state-12991/README.md). This addition does not promote the chapter’s remaining teaching or qualification scope.

## Compiled actor adoption (#12992)

Actor Lab composes the existing State Lab clock with six compiled phases: lifecycle, sensing, choice, shared walking, finalized poses and presentation. All scoped transitions preflight before any phase publishes; reads and drawing add no clock. See [the actor guide](../actors/README.md) and [qualification](../../docs/verification/actors-12992/README.md). This bounded adoption does not promote the rest of this chapter.
