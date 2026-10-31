# Genesis parity plan

Status: planned, 2026-10-31. Nothing in this plan runs yet.

The parity target is [Genesis World 1.4.5](../../docs/research/genesis/genesis.md),
a GPU physics platform for robotics and embodied AI, and its compiler,
Quadrants (see the [technical report](../../docs/research/genesis/technical-report-2026-10.md)).
"More or less parity" means the same classes of capability, each with its own
acceptance checks, not the same API or the same numbers: rigid and
articulated dynamics with contact, five material solvers and their coupling,
batched environments, selective gradients, control, sensors, observation
rendering, assets, replay and an honest performance account.

The work splits along the existing boundary between the two projects.

- **Coax** supplies language and compiler features: logical tensors separate
  from physical layout (R21), compile and startup budgets with a kernel cache
  (R22), device graphs and zero-copy buffers (R23), and kernel derivatives
  (R24), on top of the physical-world foundations already planned (R12–R20).
  See [Coax roadmap](../../coax/ROADMAP.md) sections 24–27 and spec stage 11.
- **Psionic** supplies the engine: solvers, coupling, batching, sensors,
  rendering hooks, assets, replay and validation, as book Part XVIII
  (chapters 165–185), with classical control in 188 and batched policy
  learning in 192.

## Parity map

| Genesis capability | Psionic chapter | Coax phase |
| --- | --- | --- |
| Scene, entities (morph, material, surface), build/step lifecycle | [165](../book/ch165-a-scene-owns-the-state.md) | R5, R16, R20 |
| Free rigid bodies, integrators (`approximate_implicitfast` and others) | [166](../book/ch166-rigid-bodies-and-time-steps.md) | R12, R15 |
| Articulations in generalized coordinates, CRBA mass matrix, joints including screw | [167](../book/ch167-joints-in-generalized-coordinates.md) | R12, R17 |
| Broad phase: AABB, sweep and prune, all-pairs on GPU, collision masks | [168](../book/ch168-find-the-pairs-that-might-touch.md) | R15, R21 |
| Narrow phase: MPR, GJK/EPA, primitives, SDF, multi-contact | [169](../book/ch169-find-where-they-touch.md) | R12, R15 |
| Soft-constraint convex contact solver, Newton/CG, friction cones, islands | [170](../book/ch170-contact-as-an-optimization.md) | R15, R21, R23 |
| Particle substrate (hashing, neighbors) | [171](../book/ch171-particles-and-their-neighbors.md) | R15, R21 |
| SPH liquids (WCSPH, DFSPH) | [172](../book/ch172-liquids-from-particles.md) | R21, R23 |
| PBD/XPBD cloth, rope, soft particles | [173](../book/ch173-cloth-and-rope-from-constraints.md) | R21, R23 |
| MPM (elastic, plastic, sand, snow, muscle, CPIC) | [174](../book/ch174-sand-snow-and-clay-on-a-grid.md) | R15, R21, R24 |
| FEM, explicit and implicit | [175](../book/ch175-elastic-solids-on-a-mesh.md) | R21, R23, R24 |
| Stable Fluids gases | [176](../book/ch176-smoke-on-a-grid.md) | R21, R23 |
| Explicit cross-solver coupling | [177](../book/ch177-materials-meet.md) | R12, R15 |
| SAP rigid–FEM coupling; IPC | [178](../book/ch178-contact-that-holds.md) | R15, R23 |
| Batched environments (`n_envs`), per-env parameters, resets, randomization | [179](../book/ch179-many-worlds-in-one-step.md) | R20, R21, R23 |
| Selective differentiability, adjoint reusing the factorization | [180](../book/ch180-differentiate-a-simulation.md) | R24 |
| Sensors: IMU, contact, force, tactile, lidar, depth, temperature | [181](../book/ch181-sensors-read-the-simulation.md) | R11, R13, R18, R20 |
| Rendering for observations (rasterizer, ray tracer, batch renderer) | [182](../book/ch182-pictures-for-policies.md) | R9, R21, R23 |
| Assets: URDF, MJCF, meshes, decomposition, tetrahedralization (USD later) | [183](../book/ch183-bring-in-robots-and-meshes.md) | R12, R17, R20 |
| Scene archives and `gs replay` | [184](../book/ch184-record-and-replay-a-world.md) | R1, R15, R16 |
| Compatibility matrix and performance methodology | [185](../book/ch185-measure-the-engine-honestly.md) | R0, R22 |
| PD control, IK, RRT-Connect planning | [188](../book/ch188-command-a-robot.md) | R12, R14 |
| Locomotion with PPO in thousands of environments | [192](../book/ch192-learn-a-gait-in-many-worlds.md) | R14, R20, R21, R23 |
| Quadrants: fields versus ndarrays, layouts | — | R21 |
| Quadrants: fastcache, startup and compile time | — | R22 |
| Quadrants: GPU graphs, device-side loops | — | R23 |
| Quadrants: zero-copy interop with learning frameworks | — | R23 |
| Quadrants: kernel autodiff | — | R24 |

## Order

Rigid first: 165–170, then 179 (batching), 183 (assets), 184 (replay) and 188
(control), which together replace MuJoCo as the simulator behind chapter 191.
Then the particle and mesh solvers 171–176 in any order after 171, coupling
177–178, gradients 180, sensors and observations 181–182, and 185 throughout.
Coax R21 should land when the first batched solver exists, R22 when kernel
counts make compile time visible, R23 when a contact solver loop or a
policy's observation read is measured as the bottleneck, and R24 when chapter
180 has a kernel whose hand-written reverse is the limiting factor.

## Where we deliberately differ

- **Determinism and replay.** Genesis's replay plays back recorded states.
  We also recompute from inputs and require digests to match bit for bit on
  the recording target, with batch invariance (an environment in a batch
  equals itself alone). Atomic GPU reductions are a separately named,
  non-reproducible profile.
- **Verification standards.** Every chapter has independent oracles
  (analytic solutions, brute-force references, finite differences, frozen
  recordings of other engines), admission and refusal code is `#authority`
  with zero mutation survivors, and solver iteration limits are reported,
  never silent.
- **Refusal instead of stubs.** Unsupported combinations of solver, backend,
  coupler, gradient, renderer and precision are refused at build from a
  published matrix; a gradient through a solver without one is refused at
  compile time, not returned as zero.
- **No Python.** Scenes, solvers, learners and tooling that ship are Coax;
  Python stays test tooling. External engines (MuJoCo) are oracles through
  frozen recordings, not dependencies.
- **Targets.** Quadrants dropped AOT, its C API, mobile, WebGPU and Wasm. We
  keep native code, Metal and the browser as targets, which constrains
  precision (no f64 on Apple GPUs) and is stated per profile.
- **Measurement.** Aggregate throughput is always reported with
  per-environment throughput, compile and startup times and the scene's
  contact load.
