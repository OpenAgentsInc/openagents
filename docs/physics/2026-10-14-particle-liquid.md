# Particle liquid: CPU reference, wgpu compute, screen-space drawing

**Status:** Landed 2026-10-14 for issue
[#11660](https://github.com/OpenAgentsInc/openagents/issues/11660), port plan
item 5 of the [Genesis repo study](../research/genesis/repo.md).

The code is reimplemented from the papers, not translated from Genesis:

- Macklin and Müller, "Position Based Fluids" (2013): the density solve, the
  `s_corr` tensile term and XSPH viscosity;
- Akinci et al., "Versatile Rigid-Fluid Coupling for Incompressible SPH"
  (2012): boundary particles with volume weights and two-way coupling;
- Akinci, Akinci and Teschner, "Versatile Surface Tension and Adhesion for
  SPH Fluids" (2013): cohesion, curvature and adhesion;
- van der Laan, Green and Sainz, "Screen Space Fluid Rendering with Curvature
  Flow" (2009), with bilateral smoothing in place of curvature flow.

Genesis (genesis-world `df30b61a`, Apache-2.0) is credited as the prompt;
none of its code is translated, so no Apache notice is owed.

## CPU reference: `physics::fluid::pbf`

The reference is a deterministic `f64` solver, the gameplay authority.

- **Spacing and mass.** Particles start `2r` apart and the kernel radius is
  `h = 4r`. The mass is calibrated so that a full lattice sits exactly at
  rest density.
- **Neighbours.** A sorted spatial hash with a fixed visiting order.
- **Per step:**
  1. external accelerations (gravity, surface tension, adhesion);
  2. predicted positions;
  3. neighbours;
  4. Jacobi iterations of `lambda` and position corrections, with
     `C >= 0` and the tensile term;
  5. velocities from the position change, then XSPH (fluid and boundary).
- **Threads.** Every pass is a gather over the previous pass, so a run is
  bit for bit the same on one thread or many.
- **Boundaries.** Tank walls, static parts (`add_boundary`) and coupled
  boxes (`couple_box`) are Akinci particles with volume weights `psi`.
  - The boundary term mirrors the particle's own multiplier.
  - A body takes the opposite of the momentum its boundary particles give
    the liquid (the position push and the XSPH drag), as a force and torque
    for the next `World` step.
- **Standoff.** Boundary particles sit behind the face they represent, so
  the liquid's edge lands on the face:
  - `1.42 r` for tank walls, from the rest standoff measured at the floor
    of a 0.2 m column;
  - `1.7 r` for coupled bodies, fitted once at `r = 6.25 mm` against the
    floating-box test at three densities.

  This is a calibration, and it is named as one.
- **Pressure.** `Fluid::pressure()` recovers pressure from the summed
  multipliers, `p = -kappa rho0 sum(lambda) / dt^2`, where `kappa` is the
  kernel-gradient moment on a full lattice.

Tests (`crates/physics/src/fluid/pbf_tests.rs`; `cargo test -p physics fluid`,
about 50 s):

| Test | Scene | Result |
| --- | --- | --- |
| Dam break | 2,200 particles, 0.4 x 0.4 x 0.15 m tank, 4 s | Rest density error under 1% (0.3% at 4 s). In the last second the surface moves by less than one particle and stands within 10% of the fill height the volume implies |
| Hydrostatic pressure | 1,408 particles in a 0.18 m column | `dp/dy` within 5% of `rho g` (0.9% measured); zero within a particle of the surface |
| Floating box | 4,232 particles; 120 x 75 x 120 mm boxes of 250, 500 and 750 kg/m^3 | Draft within 5% of Archimedes (measured 2.4%, 2.6% and 1.0%); at rest and upright |
| Droplets | Two 4 cm balls 1.6 cm apart, zero gravity, `gamma = 0.2` | Coalesce into one round drop within 3 s; without surface tension they never join |
| Adhesion | A drop just off a plate, zero gravity | Pulled onto the plate |
| No-slip | A film on a vertical plate | Slides less than half as far as on a free-slip wall |
| Determinism | Dam break with surface tension, 90 steps | Identical bits on one thread and many, and across two runs |

## GPU: `verse_pbr::fluid::GpuFluid`

The GPU path is a wgpu compute port of the same scene, in `f32`, for
visuals and film. It takes the CPU fluid's tank, wall particles, mass,
kernel and settings.

- **Per step:** predict; build a counting-sort grid (atomic counts, a blocked
  prefix scan, scatter); run the even number of Jacobi `lambda`/`delta`
  iterations ping-ponging two position buffers; then velocity and XSPH.
- **Bindings.** Five storage buffers and one uniform, within the default
  limit of 8 storage buffers per stage.
- **Not bit-identical.** Atomics decide the summation order, so runs differ
  in their last bits. The GPU path is checked against the CPU statistically.
- **CPU-only for now:** surface tension, adhesion and rigid coupling.

Checks (`crates/verse-pbr/src/fluid/tests.rs`):

- **Dam break, GPU against CPU**, at 0.25, 0.5, 1, 2 and 4 s.
  - Tolerances: centre of mass within 15 mm while the wave runs and 5 mm
    settled; density error within 0.5 points of the CPU's; settled density
    error under 1%; settled surface within 5 mm.
  - Measured on the M5 Max: centres of mass within about 1 mm at every
    sample, and density error 0.25% for both at 4 s.
- **50k particles at 60 fps** (`--include-ignored`): 60,000 particles,
  6 iterations, one step per frame, submitted and waited each frame:
  **4.4 ms per frame (230 fps)** on the Apple M5 Max, measured while the
  machine was heavily loaded by other builds.
- **Shaders.** Both shaders are validated with naga in the test suite.

## Drawing: `verse_pbr::fluid::render::ScreenSpaceLiquid`

The liquid is drawn as a screen-space surface:

1. **Splats.** Particles are drawn as sphere impostors into a view-depth
   target, and their thicknesses are summed into a second target.
2. **Smoothing.** Depth gets two rounds of separable bilateral smoothing.
   The smoothing is sized to the projected particle and cut off across
   depth jumps, so separate sheets stay separate.
3. **Shading.** A full-screen pass rebuilds normals from the nearer of
   each neighbour pair. It adds Schlick Fresnel reflection of a sky, the
   refracted scene behind with Beer-Lambert absorption from the thickness,
   and a sun highlight. Pixels where the scene is nearer show the scene.

Offline film surfaces through `splashsurf_lib` were optional in the plan
and are not added. Screen-space drawing serves both the live view and the
demo, so there is no new dependency to record.

## Demo

```sh
cargo run --release -p verse-pbr --example liquid_film -- OUT_DIR [SECONDS] [WIDTH HEIGHT]
```

The demo is an original scene:

- a 2.5 s pour from a spout onto the shoulder of a bottle-shaped part,
  whose surface of revolution is sampled as boundary particles;
- the water runs down the bottle and pools in a 28 cm tray;
- the CPU reference simulates it, with surface tension 0.15, adhesion 6
  and boundary viscosity 0.08, at `r = 4 mm`, rising to 4,875 particles.

1080p frames and an MP4 are in `~/Downloads/liquid-pour-11660/` on the
owner's Mac (6 s, 180 frames, about 45 s to simulate and render).

## Live in the Water Lab

The Water Lab has a glass tank on a stand, ahead of and to the right of the
spawn (`cbb573740b`).

- **What it does.** Every seven seconds the tank lets a column of about
  1,200 particles go, alternating ends. The water breaks, runs, climbs the
  far wall and sloshes.
- **How it is drawn.** On Medium and High, the physical renderer draws it
  with `ScreenSpaceLiquid` after the water and blended passes. It refracts
  the opaque scene's copy and is hidden behind its depth copy;
  `render.wgsl`'s composite mode handles reversed depth.
- **Who steps it.** Verse's scene device asks for WebGL 2 limits, with no
  compute. So the CPU solver steps the tank on its own thread (inline on
  the web), and the renderer uploads the latest positions. The GPU solver
  is used where the device can compute.
- **How to see it.** Run `verse --water-lab` and walk to the stand.
  Offscreen, run
  `cargo run -p verse --features capture --example water_capture -- OUT_DIR --only tank`.
- **Capture.** `~/Downloads/water-lab-tank-11660.png` on the owner's Mac.
