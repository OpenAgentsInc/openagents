# Genesis repo study: what is real, and what we port to Rust

Date: 2026-10-14
Follows: [promo-video.md](promo-video.md) (the shot-by-shot study, its
capability inventory and ranked gap list). Earlier mechanism ports:
[Genesis port roadmap](../../physics/2026-09-27-genesis-port-roadmap.md)
(#9788, GP-0 to GP-8, all landed).

We read the code. We did not install or run any of it.

## Pinned references

All are read-only clones in the workspace under `projects/repos/` and are
tracked in `projects/manifest.txt`.

| Repo | Pinned commit | Licence | Role |
| --- | --- | --- | --- |
| [Genesis-Embodied-AI/genesis-world](https://github.com/Genesis-Embodied-AI/genesis-world/tree/df30b61a07c3bef3e8d53abbfa7f1c367ca77384) (checkout `Genesis`; the old `Genesis` URL redirects) | `df30b61a07c3bef3e8d53abbfa7f1c367ca77384` (v1.4.4) | Apache-2.0, no NOTICE file | The engine: Python plus Quadrants kernels |
| [Genesis-Embodied-AI/quadrants](https://github.com/Genesis-Embodied-AI/quadrants) | `4f133a3ddeb964406564e81958815e1c1d870d35` | Apache-2.0, no NOTICE file | GPU kernel compiler, a Taichi fork. Genesis pins `quadrants==1.3.3` |
| [Genesis-Embodied-AI/genesis-nyx](https://github.com/Genesis-Embodied-AI/genesis-nyx) | `d8d61f2` | Apache-2.0 for the docs and examples | The Nyx path tracer. The renderer itself ships only as prebuilt CUDA wheels, with no source |
| [Genesis-Embodied-AI/gs-madrona](https://github.com/Genesis-Embodied-AI/gs-madrona) | `7c4f160a` | MIT | Fork of the Madrona batch renderer, for many-environment cameras |
| [Alif-01/LuisaRender](https://github.com/Alif-01/LuisaRender) | `030a375f` | BSD-3-Clause | The ray tracer Genesis uses, a submodule at `genesis/ext/LuisaRender` |
| [ACMLCZH/ParticleMesher](https://github.com/ACMLCZH/ParticleMesher) | `006bb10` | **None** (no licence file) | OpenVDB particle-to-surface mesher; also a submodule |

Genesis paths below are relative to the `genesis-world` root at the pinned
commit.

## Summary

- **The physics is real and large.** Genesis has rigid, MPM, SPH, PBD, FEM,
  stable-fluid smoke, tool and kinematic solvers. Three couplers join them.
  Robot loading, batched environments, sensors including tactile, IK and debug
  drawing are all present. It runs on about 393 `@qd.kernel` GPU kernels.
- **The rigid solver is a MuJoCo port in Quadrants.** It uses generalized
  coordinates, CRBA, a sparse LDLᵀ factorisation and MuJoCo's primal CG or
  Newton soft-constraint solver. Collision uses MPR or GJK/EPA. There is a
  parity test suite against the `mujoco` library. We already bind MuJoCo in
  `crates/robot-sim`, so we do **not** port this solver. We port its collision
  pieces into our own maximal-coordinate engine.
- **Nothing generative exists.** No `gs.generate`, no LLM client, and no
  prompt handling anywhere in the repo. The promo notebook is a storyboard.
- **The speed claim is env-steps per second.** It counts a Franka on a plane
  over 30,000 environments on a GPU, which gives "43M FPS". At `dt = 0.01`,
  that is the "430,000x real time". None of the checked-in benchmarks turn on
  hibernation.
- **The photoreal shots came from an optional renderer path.** That path is
  LuisaRender (BSD-3, CUDA, DirectX or Metal), now joined by the closed Nyx
  plugin. The bundled rasteriser is a vendored pyrender.
- **What we take:**
  - convex-hull and SDF collision with offline convex decomposition;
  - XPBD cloth;
  - a CPU-first batched-environment harness with an honest benchmark metric;
  - PBF liquid, as a CPU reference first and then wgpu;
  - the scene, morph, material and surface split as the shape of our typed
    scene document;
  - the debug-arrow and camera-output API shape for film and sensors.

  All of these are algorithm reimplementations in Rust. None is a translation
  of Python or Quadrants code.

## Architecture

### Module map

| Area | Path | Notes |
| --- | --- | --- |
| Entry, backend pick | `genesis/__init__.py`, `genesis/constants.py`, `genesis/utils/misc.py` | `gs.init(backend=...)`. GPU tries CUDA, then AMDGPU, then Metal, then CPU |
| Scene and simulator | `genesis/engine/scene.py`, `genesis/engine/simulator.py` (628 lines) | `scene.add_entity(morph, material, surface)`, then `scene.build(n_envs=...)` |
| Solvers | `genesis/engine/solvers/` | Listed below |
| Couplers | `genesis/engine/couplers/` | `legacy_coupler.py`, `sap_coupler.py`, `ipc_coupler/` |
| Entities, materials | `genesis/engine/entities/`, `genesis/engine/materials/{MPM,SPH,PBD,FEM,SF}/` | Material type picks the solver |
| Options (typed, pydantic) | `genesis/options/` (`morphs.py`, `solvers.py`, `sensors/`, `surfaces`) | Morph = geometry source; Material = physics; Surface = look |
| Sensors | `genesis/engine/sensors/` | Cameras, IMU, contact, joint torque, tactile, raycaster |
| Rendering | `genesis/vis/` (`rasterizer*.py`, `raytracer.py`, `batch_renderer.py`, `camera.py`), `genesis/ext/pyrender/` | Three back ends |
| Differentiability | `genesis/grad/`, solver `*_grad` methods, `rigid/abd/diff.py`, `rigid/abd/manual_bw.py` | Torch `Tensor` subclass and checkpoint replay |
| Parsers | `genesis/utils/urdf.py`, `genesis/utils/mjcf.py`, `genesis/utils/usd/`, `genesis/ext/urdfpy/` | MJCF goes through the `mujoco` compiler |
| Recording | `genesis/recorders/` | Video, CSV, NPZ and live plotters |

### Solvers

| Solver | File (lines) | Method |
| --- | --- | --- |
| Rigid | `engine/solvers/rigid/` (about 39,500 in total; `rigid_solver.py` 3,696) | Generalized coordinates: CRBA mass matrix, RNE bias force, tree-sparse LDLᵀ. MuJoCo-style primal soft-constraint solve with Newton (the default) or CG, `sol_params` as solref/solimp, and a pyramidal or elliptic cone. Integrators: Euler, `implicitfast` and `approximate_implicitfast` (the default) |
| MPM | `mpm_solver.py` (1,345) | MLS-MPM with APIC transfer. Materials: elastic (corotated or neo-Hookean), elastoplastic, snow, Drucker-Prager sand, liquid and muscle |
| SPH | `sph_solver.py` (993) | WCSPH (the default) or DFSPH, cubic kernel, viscosity `mu` and surface tension `gamma` |
| PBD | `pbd_solver.py` (1,088) | XPBD stretch, bend and volume constraints. Materials: cloth, tet elastic, PBF liquid and particles. Particle hash-grid self-collision only; no cloth triangle self-collision |
| FEM | `fem_solver.py` (1,593) | Explicit by default, or implicit Newton with PCG and line search. Linear, stable neo-Hookean or linear corotated material; muscle |
| SF | `sf_solver.py` (304) | Stable-fluids smoke grid (128³, one inlet). Basic |
| Tool, kinematic | `tool_solver.py` (135), `kinematic_solver.py` (1,825) | A differentiable tool and prescribed-motion ghosts |

The simulator steps the solvers in a fixed order: tool, rigid, kinematic,
MPM, SPH, PBD, FEM, SF. Every solver shares one substep rate, with
`substep_dt = dt / substeps`. Each substep runs four stages:

1. coupler `preprocess`;
2. each solver's `substep_pre_coupling`;
3. coupler `couple`;
4. each solver's `substep_post_coupling`.

If the rigid solver is the only one, a fast path skips coupling.

### GPU backend

Quadrants is Taichi, forked in June 2025, renamed from `gstaichi`, and
trimmed: there is no GUI, AOT, OpenGL or mobile support. Kernels are Python
functions decorated `@qd.kernel` and `@qd.func`. The outermost loop runs in
parallel over `qd.field` or `qd.ndarray` storage, with atomics,
`qd.ad.Tape` autodiff and subgroup ops. The example cited is
`kernel_substep_dynamics` in `rigid_solver.py`.

Back ends:

- CPU, CUDA and AMDGPU go through LLVM.
- Vulkan and Metal go through SPIR-V. Metal translates that with SPIRV-Cross
  to MSL at run time.

**On Apple, Metal is the recommended path, but it is 32-bit only.** It has no
`f64` and no 64-bit atomics. Bounds checks and asserts are off. Genesis
refuses `precision="64"` on Metal.

Quadrants is 134k lines of C++ and 39k of Python.

## Claims vs. code

Status key: **Implemented** means it exists and is used, **Partial** means
some of it exists, and **Absent** means the promo shows it but the code does
not have it.

| Promo claim | Status | Where (at `df30b61a`) | Notes |
| --- | --- | --- | --- |
| Rigid bodies | Implemented | `engine/solvers/rigid/rigid_solver.py`, `rigid/abd/forward_dynamics.py` | A MuJoCo port; parity tests in `tests/rigid/test_mujoco_parity.py` |
| Articulated robots | Implemented | `rigid/abd/*`, `rigid/constraint/solver.py` (5,927) | Reduced coordinates, joint limits, frictionloss, and connect/weld/joint equalities |
| MPM (frosting, foam tearing, sand) | Implemented | `mpm_solver.py`, `materials/MPM/` | Viscoplastic frosting would come from elastoplastic or liquid tuning. Tearing comes from plasticity, not explicit fracture |
| SPH (droplet, surface tension, viscosity) | Implemented | `sph_solver.py`, `materials/SPH/liquid.py` | The promo's "microscopic mode" clay spheres match SPH particles |
| PBD and cloth | Implemented | `pbd_solver.py`, `materials/PBD/cloth.py` | XPBD; limited self-collision |
| FEM soft bodies, muscle (tube dancers, plant) | Implemented | `fem_solver.py`, `materials/FEM/` | Implicit FEM is needed for SAP coupling |
| SF / smoke | Partial | `sf_solver.py` | One inlet, a fixed grid and no coupling |
| Coupling between solvers | Implemented | `couplers/legacy_coupler.py` (967), `sap_coupler.py` (4,007), `ipc_coupler/` | Legacy covers rigid with MPM, SPH, PBD and FEM, MPM–SPH, MPM–PBD, FEM–MPM and FEM–SPH. SAP is Drake-style hydroelastic or point contact for rigid and FEM. IPC needs the external `libuipc` |
| Narrow phase | Implemented | `rigid/collider/narrowphase.py` (4,583), `mpr.py`, `gjk.py`, `epa.py`, `box_contact.py`, `capsule_contact.py`, `multi_contact.py` | MPR by default; GJK/EPA when gradients are on. Multi-contact by perturbed re-detection, or by an optional face-clipping patch |
| Convex decomposition | Implemented (external tool) | `utils/mesh.py` `convex_decompose`, `options/misc.py` `CoacdOptions` | Calls CoACD (MIT) and caches the result as `.cvx` |
| SDF collision | Implemented | `entities/rigid_entity/rigid_geom.py` `_preprocess`, `utils/sdf.py`, `narrowphase.py` `func_contact_vertex_sdf` | A precomputed voxel SDF per geom (via `igl.signed_distance`, up to 128³), used for non-convex pairs and terrain |
| Thousands of non-convex rigids (letters in the teapot) | Implemented, as a capability | Decomposition plus SDF contact | We did not find that scene in the examples |
| Differentiable physics | Partial | `genesis/grad/`, `Simulator._step_grad`, `tests/grad/` | MPM, rigid, explicit FEM and tool only. PBD, SPH and SF are stubs. Rigid gradients force GJK and turn off hibernation, box-box contact and the elliptic cone |
| Rasteriser | Implemented | `vis/rasterizer_context.py`, `ext/pyrender/` (a vendored fork) | OpenGL |
| Ray tracer (beauty shots) | Implemented, optional | `vis/raytracer.py` with LuisaRender | Offline-grade path tracing. Particle liquids are meshed by splashsurf (by default) or ParticleMesher (`utils/particle.py`) |
| Nyx path tracer | Plugin, closed binary | `genesis-nyx` wheels only | CUDA 12.9+ only |
| Batch renderer | Implemented, Linux x86-64 only | `vis/batch_renderer.py` with `gs-madrona` | Renders every environment and camera in one call |
| Cameras: RGB, depth, segmentation, normals | Implemented | `vis/camera.py` `render(rgb, depth, segmentation, normal)`, `engine/sensors/camera.py`, `depth_camera.py` | Segmentation is by entity, link or geom |
| Tactile | Implemented | `engine/sensors/kinematic_tactile.py`, `point_cloud_tactile.py`, `tactile_shared.py` | Taxel, probe and elastomer models |
| IMU, contact force, lidar | Implemented | `imu.py`, `contact_force.py`, `raycaster.py` | |
| Force and velocity arrows (promo shot 7) | Partial | `scene.py` `draw_debug_arrow` and others; `visualize_contact` with `contact_force_scale` | Rasteriser only, with no labels. The promo's labelled overlay is compositing |
| Multi-camera shots (shot 5) | Implemented | `scene.add_camera`, camera `start_recording` | Cameras are scripted in Python, not prompted |
| URDF | Implemented | `utils/urdf.py`, `ext/urdfpy/` | xacro is supported |
| MJCF | Implemented | `utils/mjcf.py` | Compiled by `mujoco.MjModel` and then read back |
| USD | Implemented | `utils/usd/` | Needs `pxr` |
| Batched environments | Implemented | `scene.build(n_envs=...)`, field layout `[n_items, B]` | Per-env parameters need the `batch_*_info` flags |
| RL training | Partial (examples) | `examples/locomotion/go2_env.py`, `go2_train.py`, `backflip/`, `examples/manipulation/` | Uses rsl_rl PPO. There is no gym wrapper in the library |
| Text to policy, text to reward (LEAP hand crawl) | Absent | — | Rewards are written by hand, as the promo's own "human-designed reward" tag on the backflip clip says |
| `gs.generate`: text to scene, camera, force | Absent | — | No LLM, prompt or generation code anywhere. The `generate*` functions are geometry helpers |
| Hibernation | Implemented, experimental | `rigid/abd/forward_dynamics.py` `func_count_settled_step`, `rigid/abd/misc.py` `func_hibernate_island_if_settled` | Off by default. Sleeps a whole island after 10 still steps, and wakes it on contact, setters or actuation |
| "10–80x faster", "430,000x real time" | Narrated | `examples/speed_benchmark/franka.py` (comment: "43M FPS"), `anymal_c.py` ("14.4M FPS"), `tests/benchmarks/test_rigid.py` | Metric: `num_steps * n_envs / time`. 30,000 environments of a single robot on a plane. Hibernation is not on in any checked-in benchmark |
| Sim-to-real | Absent in repo | — | The real-robot clips are external |
| Fish, faces, dancers (montage) | Absent | — | Not in this repo |

## Port plan

The rules are those of the earlier roadmap. Each mechanism is **reimplemented
in Rust** in the crate that owns it, deterministic and in `f64` on the CPU
first. It is proven by a test there, and zones consume it.

- No Genesis Python, Quadrants kernels or assets enter the repo.
- Genesis stays a reference and an offline oracle
  (`crates/physics/oracle/genesis_oracle.py`).
- Where a GPU path follows, it is wgpu compute (`wgpu` 30 is already in
  `crates/verse-pbr`). It must sit behind a CPU reference with the same
  test scene.

Sizes: **S** is days, **M** is one to two weeks, **L** is about a month.

Issues filed for the top five (all in project 22):
- #11656: hulls;
- #11657: the SDF collider and decomposition bake;
- #11658: cloth;
- #11659: batched environments;
- #11660: PBF liquid.

| # | Port | What it is in Genesis | Lands in | Approach | Size | Deps | Gap list rank |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | **Convex-hull colliders: MPR plus GJK/EPA, and a face-clipped manifold** | `collider/mpr.py`, `gjk.py`, `epa.py`, `multi_contact.py`, `support_field.py` | `crates/physics` (`collision.rs` gains a `Shape::Hull`; new `collision/hull.rs`, `gjk.rs`, `mpr.rs`) | Reimplement from the published algorithms: MPR (Snethen, XenoCollide), GJK, EPA, and reference/incident face clipping like our box–box. We use Genesis as an oracle for contact sets, not as source | M | None new (`glam`) | 4 |
| 2 | **Static SDF collider, plus an offline convex-decomposition bake** | `rigid_geom.py` `_preprocess`, `utils/sdf.py`, `func_contact_vertex_sdf`; `utils/mesh.py` `convex_decompose` with CoACD | `crates/physics` (`Shape::Sdf` grid, static only, vertex-versus-SDF contact). The bake goes in the CAD and Blender asset pipelines (`crates/cad` export, `docs/verse/blender-pipeline.md`) | Reimplement. The SDF grid is computed offline and stored in a versioned file. CoACD runs as an offline tool; we never link it into the engine | M | CoACD as an offline CLI (MIT) | 4 (the teapot, containers) |
| 3 | **XPBD cloth** | `pbd_solver.py` stretch and bend with compliance, particle hash-grid collision, particle-to-link attach; `materials/PBD/cloth.py` | `crates/physics` `cloth.rs`, next to `rope.rs`; drawn in `verse-pbr`; first demo in a zone | Reimplement (Macklin et al. XPBD), reusing the rope constraint loop. Collide against our primitives and hulls. Pin to bodies with two-way impulses | M | None | 5 |
| 4 | **Batched environments, with an honest env-steps/s benchmark** | `scene.build(n_envs)`, the `[n, B]` layout, `batch_*_info` flags, the `test_rigid.py` metric | `crates/robot-sim` (N MuJoCo `mjData` over one `mjModel`, stepped across threads); `crates/physics` (`BatchWorld` over `parallel.rs`) for native scenes | An idea port. Per-env randomised parameters are explicit. Report `steps × envs / s` **and** real time per env, in active, contact-rich scenes, never idle ones | M | MuJoCo already bound | 9 (first half) |
| 5 | **PBF liquid: a CPU reference, then wgpu compute** | `pbd_solver.py` liquid (poly6/spiky kernels, lambda density solve, `S_corr`, viscosity); `sph_solver.py` surface tension `gamma` | `crates/physics` `fluid/pbf.rs` (CPU, small tank); then a wgpu compute path in `crates/verse-pbr`; offline film surfaces through the `splashsurf_lib` crate (Rust, MIT) | Reimplement (Macklin and Müller, PBF 2013), with a deterministic spatial hash. GPU second, matched against the CPU scene | L | `splashsurf_lib` (MIT) for film only | 8 |
| 6 | **Typed `scene.v1` in Genesis's shape** | `add_entity(morph, material, surface)`, the `options/morphs.py` source kinds, `force_fields.py` (constant, wind, point, drag, noise, vortex, turbulence), `scene.add_camera` | A new scene-document crate consumed by `crates/physics` and `crates/verse-film`, following the Game Lab and CAD Lab JSON Patch pattern | Schema idea only: separate geometry source, physics material and look. Timed forces and camera directives are added as typed events | M | None | 6 |
| 7 | **Debug arrows and camera outputs** | `draw_debug_arrow`, `visualize_contact`/`contact_force_scale`, `camera.render(rgb, depth, segmentation, normal)` | `crates/verse-film` (a labelled annotation layer from recorded telemetry); `crates/verse-pbr` (depth, segmentation and normal targets) | API shape only. We add labels, which Genesis lacks | S | None | 1, 10 |
| 8 | **Batched damped-least-squares IK** | `rigid/abd/inverse_kinematics.py`, `inverse_kinematics_multilink` | `crates/robot-sim` (Jacobians from MuJoCo `mj_jac`) | Reimplement (DLS with joint limits and multiple links) | S | None | Robot Lab IK gap |
| 9 | **Legged robots and hands in Robot Lab**, rewards from Go2 | `examples/locomotion/go2_env.py` reward terms, PD action scaling, resampled commands | `crates/robot-sim`, `crates/verse-zone-robotlab` | Rewrite the reward and observation design. Load licensed models (MuJoCo Menagerie), not Genesis assets: their go2 and shadow_hand ship without licence files | M | MuJoCo | 7 |
| 10 | **Tactile taxels** | `kinematic_tactile.py` taxel and probe sensors, `tactile_shared.py` noise and hysteresis | `crates/physics/src/sensors.rs` | Reimplement over contact queries | M | None | 14 |
| 11 | **MLS-MPM (wgpu)** | `mpm_solver.py` | GPU only; a later crate | Reimplement from MLS-MPM (Hu et al. 2018) | L | wgpu | 13 |

### Landed

- **#11656, hulls.** `Shape::Hull` in `crates/physics/src/collision/`:
  `hull.rs` (Quickhull build with a degenerate-input policy, polygon faces,
  mass properties, `World::add_hull_body`), `gjk.rs` (GJK and EPA on
  support functions; spheres and capsules are a point or segment core plus
  a radius), and `convex.rs` (reference/incident face clipping to at most
  four points, and a consistency check that falls back to face axes when a
  barely-touching GJK normal or a degenerate EPA polytope is wrong). Tests
  in `collision/hull_tests.rs`: a stack of five hexagonal prisms rests for
  10 s within the solver slop and sleeps; 200 mixed hulls thrown down at up
  to 12 m/s settle inside a walled box; 1, 4 and 8 threads step bit for
  bit; the GP-1 ledger holds to 1e-12 through hull contacts.
  `cargo run --release -p physics --example hull_pile` on the M5 Max (with
  other builds running): 2.6 ms mean, 9.9 ms worst per step while 200
  hulls fall; 0.15 ms once they sleep.
- **#11657, distance grids and the bake.** `Shape::Sdf` (static only) in
  `collision/sdf.rs`: an `f32` grid queried trilinearly in `f64`, capped at
  128 samples per axis, in a versioned byte format; a hull's or box's
  corners, edge midpoints and face centres, or points along a capsule's
  core, sample it. `SdfGrid::bake` samples a closed mesh (nearest triangle,
  signed by the generalized winding number). The offline bake is
  `crates/cad/src/collision.rs`, the `collision` export: CoACD (MIT, run as
  `python3` with `pip install coacd`, never linked) splits each solid into
  convex pieces, recorded with its version and parameters, and the grid of
  all the solids is written beside them. Tests: 300 rocks poured into a
  baked bowl stay in its cavity; a U bracket split by CoACD (checked-in
  output, `collision/fixtures/u-bracket.coacd.json`) falls and rests as one
  compound body (`World::add_compound_body`); the same mesh or document
  bakes to the same bytes.
- **#11658, cloth.** `crates/physics/src/cloth.rs`, beside `rope.rs` and on
  its XPBD loop: stretch and shear distance constraints, isometric bending
  (Bergou et al. 2006; a dihedral-angle constraint flipped at full folds
  and pumped energy on a fine grid), pins to world points or two-way to
  bodies, contacts with every collider shape as in-pass constraints with
  position-level friction and Müller's velocity update, air drag, and
  optional particle self-collision through a sorted hash. Tests in
  `cloth/tests.rs`: a 32x32 cloth drapes over a sphere and rests (under
  5 mm/s creep, no jitter); a banner on a free body conserves momentum to
  1e-12 and a wind on it drags the body; 1, 4 and 8 threads bit for bit;
  restore bit for bit. Drawing: `verse_pbr::cloth`. Zone: the Physics Lab
  flag (`verse-zone-lab`), in every scenario.
  `cargo run --release -p physics --example cloth_drape`: 2.3 ms per
  1/120 s step for 1,024 particles at 12 substeps on the M5 Max (about
  2.2 µs per particle-step), 0.5 ms for 256. **Budget:** 256 cloth
  particles per zone (a 16x16 flag), about 0.5 ms on the M5 Max and within
  the 1 ms phone step budget at the phones' measured 2–3x slower cores;
  films and offline scenes may use more.

**What we do not port:**

- **The generalized-coordinate rigid solver.** It is a MuJoCo port, and we
  call MuJoCo directly in `robot-sim`.
- **The SAP and IPC couplers.** IPC is an external library, and both assume
  implicit FEM.
- **Differentiability**, which nothing we ship needs yet.
- **Quadrants.** It is a Python DSL with an LLVM toolchain, and on Metal it is
  32-bit with no `f64`. That rules it out for our deterministic `f64` engine
  and our phone targets.
- **LuisaRender, Nyx and Madrona.** For film we would build on `verse-bake`'s
  ray queries instead (rank 12 in the gap list).

## Licence and attribution

- **Genesis and Quadrants** are Apache-2.0 with no NOTICE file. Our repo is
  also Apache-2.0, so they are compatible.
  - **Plan:** all the items above are reimplementations from papers and
    behaviour, so no Apache obligation arises. We still credit Genesis in each
    module's doc comment and in this doc.
  - **If any Rust file is translated directly from a Genesis file**, Apache-2.0
    §4 requires three things:
    1. keep the licence (ours already is Apache-2.0);
    2. keep Genesis's copyright and attribution notices;
    3. mark the file as changed: "Derived from Genesis-Embodied-AI/genesis-world
       `<path>` at `df30b61a`, Apache-2.0; modified".

    Genesis has no NOTICE file, so there is no NOTICE text to carry. If we
    ever do translate a file, we add a root `NOTICE` entry naming Genesis.
- **Do not port from these vendored files:**
  - `genesis/ext/isaacgym/terrain_utils.py`: the header says NVIDIA
    proprietary, "strictly prohibited" to reproduce.
  - `genesis/ext/pyrender/`, `genesis/ext/urdfpy/` and
    `genesis/ext/_trimesh_patch.py`: MIT upstream, but Genesis dropped the
    MIT notices. If we need them, we go to the upstream repos.
  - `genesis/ext/VolumeSampling`: a committed Linux binary of SPlisHSPlasH
    (MIT), shipped without a licence.
- **The other clones:**
  - ParticleMesher has no licence, so all rights are reserved. Reading it is
    fine; copying from it is not.
  - Nyx is closed (wheels only).
  - gs-madrona is MIT.
  - LuisaRender is BSD-3-Clause. Its own dependencies are unfetched
    submodules.
  - Quadrants' `external/` holds the usual mix: MIT, BSD, Apache and Eigen
    under MPL-2.0. It also has NVIDIA libdevice and ROCm device libraries. None
    of this matters, because we do not port Quadrants.
- **Assets:**
  - Licensed: `urdf/panda_bullet` and `xml/franka_sim` are Apache-2.0;
    `urdf/anymal_c` is BSD-3 (ANYbotics); `xml/universal_robots_ur5e` is BSD
    (ROS-Industrial).
  - No licence file: `go2`, `shadow_hand`, `kuka_iiwa`, `drones`, the
    PartNet-Mobility object `3763` and the meshes.

  We take models from licensed sources (MuJoCo Menagerie, the vendors), never
  from `genesis/assets`.
- **Tools we would use:**
  - CoACD (MIT) as an offline CLI;
  - the `splashsurf_lib` crate (MIT) for offline surfaces in film.

  Both would be recorded with their licence when they are added.

## Risks

- **Hull contact is where determinism breaks.** MPR and GJK branch on
  floating-point comparisons. Bit-identical replay across our thread counts
  needs fixed iteration orders and tolerances, plus a trace test like the one
  in `parallel.rs`. Cross-platform identity remains a tolerance, as
  GP-0 already states.
- **Manifold stability.** Genesis needed two multi-contact schemes. One
  re-detects on perturbed copies, which is costly; the other clips faces. We
  should pick face clipping and test a stack of hulls at rest before relying
  on it.
- **SDF memory.** A 128³ grid per geom is about 16 MB in `f64`. We keep grids
  static, cap their resolution, and store them in `f32` with an `f64` query.
- **Cloth cost on phones.** XPBD cloth with collision is iteration-bound. We
  budget particle counts per zone, the way rope is budgeted.
- **The GPU liquid is not deterministic across GPUs.** It belongs to film and
  visuals. Gameplay authority stays on the CPU.
- **Benchmark honesty.** Our batched number must state the active scene, the
  environment count and the hardware. We do not quote env-steps/s without
  real time per environment.
- **Scope creep toward Genesis parity.** MPM, FEM, SAP and differentiability
  are each at least a quarter of work. We only start them with a named zone
  or film shot that needs them.
