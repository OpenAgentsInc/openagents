# Genesis promo video: shot-by-shot study and what the Verse would need

Date: 2026-10-14
Source: YouTube `g7AIogKm2es`, "20+ Labs, 2 Years: The MIND-BLOWING Genesis
Physics Engine is HERE!" (channel Sentivium, uploaded 2024-12-19, 3:26,
720p30). It is a re-upload of the Genesis project's launch reel for
<https://github.com/Genesis-Embodied-AI/genesis-world> (the end card points at
the older `Genesis-Embodied-AI/Genesis` URL).

The video was downloaded for private analysis only (local folder
`~/work/film-out/research/genesis-promo/`, never committed). Descriptions
below are ours; no frames are reproduced and on-screen text is quoted only in
short lines. Method: ffmpeg scene detection (`gt(scene,0.3)` and `0.15`),
one frame a second on 4x4 contact sheets, full-resolution stills at key
moments, and a whisper.cpp (`ggml-small.en`) pass over the audio.

The repo itself is studied separately. Earlier Genesis notes:
`docs/physics/2026-09-27-genesis-for-verse-zones.md` and
`docs/physics/2026-09-27-genesis-port-roadmap.md` (#9788).

## Summary

There is no narration. The soundtrack is music, plus a typewriter sound under
the typed prompts. Every claim is made through on-screen text, a mock Jupyter
notebook with `import genesis as gs` and `gs.generate("...")` calls typed in
plain English, and title cards.

The reel has four acts.

1. **Prompt to physical world (0:00–1:20).** A one-line prompt produces a
   photoreal macro shot: a water droplet sliding down a cold green beer bottle.
   Further prompts then re-shoot it:
   - three named camera moves;
   - a labelled force and velocity overlay with live values;
   - a "microscopic mode" that turns the droplet into grey spheres, with arrows
     for surface tension and viscosity.

   The claim is that this is "a real 4D physical world: interactive, dynamic,
   fully controllable, and driven by real physics".
2. **More generated worlds (1:20–1:54).** A fast run of material showcases:
   - frosting piped onto a cake;
   - a foam block twisted until it tears;
   - fruit dropped on marble;
   - a water jet hitting a strawberry;
   - honey coiling as it pours;
   - four inflatable tube dancers;
   - a ball of water flying at the camera through a photoreal room;
   - a glass teapot filled with falling letter-shaped pieces.
3. **Control and policies (1:54–3:02).** Prompts apply forces to a potted
   plant. The reel then cuts to robots:
   - a dexterous hand lifting a glass from a cloth;
   - two arms at a running kitchen tap;
   - a prompt that trains a dexterous hand to "walk like a quadruped", shown as
     grids of training rollouts by epoch.

   The same prompt is then shown "for different embodiment" (hand, dog,
   humanoid), with real-robot clips: "...and directly deploy to real", parkour
   and backflips on a Unitree-style quadruped, and a humanoid walking.
4. **Speed and credits (3:02–3:26).** Bar charts compare frames per second for
   PyBullet, MuJoCo, Isaac Gym and Genesis on three scenes. A banner claims
   "Up to 10~80x faster than existing GPU-accelerated simulators" and
   "430,000 faster than in real-time".

   The chart footnote qualifies the headline bar: it was measured with
   `hibernation = True`, a feature that was "under testing" for version 0.1.1.
   Every bar was run on an i9-14900KF and an RTX 4090.

   A 16-tile montage credits "all the collaborators in the community", then
   the open-source URL.

**What is actually evidenced vs. narrated.** The prompt-driven notebook is a
storyboard device: the reel never shows the generation step running, only
typed text followed by finished renders. The beauty shots look ray-traced and
offline, with heavy depth of field and caustics in glass, and are clearly not
real time. The robot and speed sections are the most concrete parts: training
grids, side-by-side sim and real footage, and benchmark charts with stated
hardware.

## Shot list

We count 45 shots, including title cards. Times are mm:ss. Scene detection
found hard cuts at 1:25, 1:31, 1:35, 1:38, 1:42, 1:48, 2:12, 2:18, 2:51, 2:53,
2:56, 2:58 and 3:15. The first 80 s are joined by dissolves and push-ins, so
those boundaries come from the 1 fps sheets.

| # | Time | What is shown | Rendering | Camera | UI / text / prompt | Claim |
|---|------|---------------|-----------|--------|--------------------|-------|
| 1 | 0:00–0:02 | Logo on black | Flat title | Static | Genesis wordmark | — |
| 2 | 0:02–0:12 | Notebook, prompt typed | Dark-purple Jupyter-style terminal | Static, then a push-in on the text | Prompt: a water droplet drops onto a beer bottle and slides down its surface | Text-to-scene |
| 3 | 0:12–0:20 | Green beer bottle, label visible, condensation beads; one droplet slides | Photoreal, looks path-traced: glass transmission, emissive-looking green rim, very shallow depth of field | Slow orbit and creep along the bottle | — | Generated 4D world |
| 4 | 0:20–0:23 | Title card | Text | — | "a real 4D physical world" | Interactive, controllable, real physics |
| 5 | 0:23–0:36 | Three vertical panels, each typing a camera prompt, then each panel's render | Same photoreal look | A: orbit while descending; B: close-up from above tracking the droplet; C: side camera tracking | "Camera View A/B/C" headers and per-panel prompts | Prompt-directed cinematography |
| 6 | 0:37–0:42 | Prompt [3] typed | Terminal | Static | Visualize all forces on the droplet and its velocity | Physics introspection |
| 7 | 0:43–0:51 | The droplet with four arrows: support force, friction, gravity, velocity. Values in mN and cm/s update every frame | Photoreal plus 2D-styled labels; arrows drawn in 3D | Tracking macro | Live numeric labels | Simulator exposes true forces |
| 8 | 0:52–0:58 | Prompt [4] typed | Terminal | Static | At 2.5 s, zoom to a "microscopic mode" showing the molecular representation | Multi-scale view |
| 9 | 0:58–1:01 | Back to the bottle, zoom in | Photoreal | Fast push-in | — | — |
| 10 | 1:01–1:05 | The droplet as a hemisphere of grey spheres on a grey plane; brief wireframe flash; an inset; up-arrows at the contact | Clay render: grey, soft shadows | Slow push | "Support Force" label | Particle (SPH-like) representation |
| 11 | 1:05–1:08 | A ring of spheres around one particle; outward arrows | Clay | Close, static | "Surface Tension" | Surface-tension model |
| 12 | 1:08–1:12 | The same cluster; downward drag arrows | Clay | Static | "Viscosity" | Viscosity model |
| 13 | 1:12–1:14 | Pull out through the particle field to an untextured clay bottle with droplets | Clay plus motion blur | Fast pull-out | — | The same sim drives the beauty render |
| 14 | 1:14–1:20 | Beauty render again; the droplet leaves a coloured trail down the label; fade | Photoreal | Tilt down | — | — |
| 15 | 1:20–1:22 | Title card | Text | — | "More generated 4D physical worlds" | — |
| 16 | 1:22–1:25 | Chocolate frosting piped in ridges onto a loaf cake | Photoreal food lighting | Slow lateral | — | Viscoplastic fluid (MPM-style) |
| 17 | 1:25–1:31 | A pink foam block held in two clear acrylic clamps, twisted until it tears into chunks | Photoreal, studio white, transmission in the clamps | Static wide | — | Elastoplastic solid, fracture |
| 18 | 1:31–1:33 | Oranges, lemons and limes dropped onto black marble, bouncing and rolling | Photoreal, mirror-like reflections | Static | — | Rigid or soft contact |
| 19 | 1:33–1:35 | Slow-motion water jet hitting a strawberry on black; spray sheets | Photoreal, high-speed look | Static | — | Liquid–solid coupling |
| 20 | 1:35–1:38 | Amber honey poured; coils and pools on the floor | Photoreal, gold specular | Low, static | — | High-viscosity liquid |
| 21 | 1:38–1:42 | Four inflatable tube dancers with faces, flailing on a white plinth | Stylized PBR, sky background | Static | — | Actuated soft bodies |
| 22 | 1:42–1:48 | A sphere of water flies across a photoreal room (brick, curtain, bed) and bursts over the camera | Photoreal, refraction | Camera rushes in, liquid covers the lens | — | Large-scale liquid, interaction with the camera |
| 23 | 1:48–1:54 | Pastel letter-shaped pieces spelling the product name pour into a glass Utah teapot, fill it and spill over | Photoreal glass and caustics | Static, then dims | — | Thousands of non-convex rigid bodies in contact |
| 24 | 1:54–1:56 | Title card | Text | "we don't just access ... we control it" | Controllability |
| 25 | 1:56–2:00 | Notebook | Terminal | Static | `scene = gs.generate(...)`: a potted plant on a wooden table | Scene from text |
| 26 | 2:00–2:08 | A plant in a pot on a wet wooden table by a window; a prompt applies a sideways force at the top for 1.5 s; an arrow appears; the plant sways | Photoreal, possibly composited on a plate | Static | Force prompt shown beside the view | Prompt-applied forces on a deformable plant |
| 27 | 2:08–2:12 | The same view; the base rotates back and forth (curved arrow) | Photoreal | Static | Prompt: rotate the plant's base back and forth for 3 s | Kinematic drive of a body |
| 28 | 2:12–2:16 | Title card | Text | "we generate robot policies that can too" | Policy generation |
| 29 | 2:16–2:18 | A dark dexterous hand on a wrist mount lifts a glass resting on a red-checked cloth over a blue cloth | Photoreal, plate background | Static | — | Manipulation with cloth |
| 30 | 2:18–2:23 | Two white arms at a black-marble kitchen sink; one works the tap and water runs | Photoreal | Static | — | Manipulation plus liquid |
| 31 | 2:23–2:27 | Notebook | Terminal | Static | Prompt: a LEAP hand walks forward like a quadruped on its four fingers | Text-to-policy |
| 32 | 2:27–2:36 | 3x3 grids of rollouts labelled Epoch 0, 50 and 100; the hand learns to crawl | Simple raster, dark grid floor | Static per tile | Epoch labels | RL from a text reward |
| 33 | 2:36–2:38 | Title card | Text | "Same text description ... different embodiment" | Embodiment-agnostic |
| 34 | 2:38–2:43 | Split screen: the hand vs. a quadruped, both walking | Raster, grid floor | Tracking | Caption: walks forward like a "dog" | — |
| 35 | 2:43–2:46 | Split: the hand vs. a humanoid | Raster | Tracking | Caption: like a "human" | — |
| 36 | 2:46–2:48 | A real humanoid walking in a corridor | Live action | Handheld | "...and directly deploy to real" | Sim-to-real transfer |
| 37 | 2:48–2:50 | Title card | Text | "auto-generated complex skills & transfer to real-world" | — |
| 38 | 2:50–2:52 | A small robot moving through a field of splashing particles on a checker floor; a humanoid stepping on a grid | Raster | Static | — | Locomotion on deformable or liquid terrain |
| 39 | 2:52–2:56 | Top: dozens of quadrupeds training in parallel on a grid. Bottom: a real quadruped on leaves and in a lobby, standing on its front legs | Raster plus live action | Static | — | Massively parallel RL, transfer |
| 40 | 2:56–3:02 | A white quadruped flipping in sim; a real one backflipping and jumping near a large ball on a campus lawn | Raster plus live action | Static, low | Small tag: "human-designed reward" | Agile skills |
| 41 | 3:02–3:03 | Title card | Text | "All at an unprecedented speed" | — |
| 42 | 3:03–3:11 | Bar charts, FPS on three scenes (a plane with a Franka arm, with a standing Anymal C, with ten 6-DoF meshes) for PyBullet, MuJoCo and Isaac Gym, then Genesis with 10x, 20x and 81x markers | Chart animation | — | Banner "Up to 10~80x faster..."; footnotes on hibernation and hardware | Speed |
| 43 | 3:11–3:14 | Title card | Text | "generative and simulation capabilities, thanks to all the collaborators" | Community |
| 44 | 3:14–3:23 | 16-tile montage, listed below | Mixed | Mixed | Same caption | Breadth |
| 45 | 3:23–3:26 | End card | Text | "Physics engine fully open-source at:" plus the URL | Open source |

The 16 montage tiles in shot 44 are:
- generated furnished interiors;
- a gear and linkage mechanism;
- kitchens with a mobile manipulator;
- a labelled office corridor;
- quadrupeds in offices and on leaves;
- a talking-head face;
- a quadruped on snow;
- a school of fish;
- three dancing characters;
- a costumed character performing;
- thousands of robot instances;
- a set dining room;
- a grid of articulated object assets (drills, laptops, toasters, microwaves, a tea kettle, scissors).

## Capability inventory, with the Verse today

Status key: **Have** means it exists and works in the Verse stack, **Partial**
means some of it exists, **Missing** means none of it does. All paths are on
`main` at `b3118a4e4b`.

### Rigid, articulated and robot control

Shown: the fruit drop, the letter pile in the teapot, arms at the sink, a
dexterous hand, quadrupeds and humanoids.

- **Have:**
  - `crates/physics` is a deterministic, double-precision rigid-body engine on
    the CPU. It uses sequential impulses with warm starting, a friction cone
    and torsional friction, sleep, a fixed step, and replay traces.
  - Joints are point, weld, tether, hinge and slider, with limits, servos and
    break limits (`crates/physics/src/joint.rs`).
  - It runs on up to 8 threads with bit-identical results
    (`crates/physics/src/parallel.rs`).
  - The Physics Lab zone (`crates/verse-zone-lab`) runs these mechanisms live.
  - Robot Lab:
    - `crates/robot-description` reads URDF with STL meshes.
    - `crates/robot-sim` (`src/mujoco.rs`) runs MuJoCo 3.3.7, qualified on
      SO-101 and Franka.
    - `crates/robot-workbench` is the workbench.
    - `crates/verse-zone-robotlab` is the Verse room.
- **Partial:**
  - Collision shapes are only sphere, capsule and box. There are no convex-hull
    or triangle-mesh dynamic colliders, so the letter pile in the teapot is out
    of reach.
  - Articulation is pairwise joints, not a reduced-coordinate solver.
    `docs/lab/lerobot-in-verse.md` scopes a spike.
  - Inverse kinematics exists only inside Lagrange 2's assembly planner
    (`crates/verse-lagrange2/src/assembly.rs`).
- **Missing:**
  - Legged robots (quadruped or humanoid) and dexterous hands.
  - MJCF authoring.
  - A general IK library.

### Soft bodies and FEM

Shown: the twisted and torn foam, the tube dancers, the swaying plant.

- **Have:** nothing volumetric. `crates/physics/src/modal.rs` gives damped
  structural modes, which Lagrange 1's flexing arrays and Lagrange 2's keel
  mode use. That is flex on a rigid frame, not a soft body.
- **Missing:** FEM or tetrahedral XPBD soft bodies, muscle actuation,
  elastoplastic tearing.

### Fluids (SPH and MPM)

Shown: the droplet, honey, frosting, the water jet, the water ball, the tap.

- **Have:** ocean-scale water.
  - `crates/physics/src/water/` has Gerstner waves and FFT seas
    (`fft.rs`, `spectrum.rs`) plus buoyancy, drag and currents (`submerge.rs`,
    `flow.rs`).
  - `crates/verse-pbr/src/water/` renders an interactive ripple and foam field,
    rain, underwater views and screen-space refraction.
  - The Water Lab zone is `crates/verse-zone-water`. Docs: `docs/verse/water.md`.
- **Missing:** particle or grid liquids (SPH, PBF, FLIP or MPM), viscous and
  viscoplastic flow, and surface tension. The droplet, honey and splash shots
  all need one of these.
- **Smoke:** sprite effects only (`crates/verse-pbr/src/fx/`,
  `docs/verse/particles.md`). There is no smoke simulation.

### Cloth

Shown: the checked tablecloth under the glass.

- **Partial:** XPBD rope (`crates/physics/src/rope.rs`). The same constraint
  machinery extends naturally to a cloth grid.
- **Missing:** cloth: stretch and bend constraints, self-collision, and the
  coupling with rigid bodies needed to drag cloth with a gripper.

### Granular

Shown: the letter pile (rigid granular). MPM sand is implied by Genesis but not
shown in this reel.

- **Partial:**
  - Many-body rigid piles of primitives work: the Meteor Showcase debris and
    the demolition yard (`crates/verse-zone-everglade/src/zones/everglade/demolition/`).
  - GPU instancing for debris is open (#10937).
- **Missing:** non-convex piles and continuum sand.

### Differentiable simulation

Not shown in the reel; it is a Genesis feature claim elsewhere.

- **Missing.** The only autodiff in the repo is the ML stack
  (`crates/psionic/crates/psionic-ir/src/autodiff.rs`).

### Speed claims

Shown: 10–81x over Isaac Gym and "430,000x real time".

- **Have:** CPU timing for one world.
  - `crates/verse-lagrange/examples/step_budget.rs` measures the rigid world at
    about 0.012 ms a step on an M5 Max.
  - `crates/physics/examples/scene_broadphase.rs` is a second benchmark.
- **Missing:**
  - Batched or vectorised environments.
  - GPU compute physics.
  - An FPS-across-many-environments benchmark comparable to their chart.

  We should not chase their headline number. It counts many idle environments
  with hibernation on.

### Generating scenes and assets from prompts

Shown: every `gs.generate` call (scene, camera, force overlay, applied forces,
policy).

- **Partial:** typed documents edited by agents.
  - Game Lab (`crates/game-lab`) edits `game.v1` by JSON Patch, checked by
    bot-played runs.
  - CAD Lab (`crates/cad`, `crates/cad-lab`) builds checked B-rep parts from
    `part.v1` and `assembly.v1`.
  - The Blender pipeline is scripted (`docs/verse/blender-pipeline.md`).
  - Video Lab is a spec plus Python tools (`docs/video-lab/README.md`,
    `scripts/film/lab.py`).
- **Missing:**
  - A typed scene document that a chat agent edits and the physics then runs:
    bodies, materials, forces over time, and camera directives.
  - Text-to-asset.

  Any text routing must follow the workspace rule: a typed semantic selector,
  not keyword matching.

### Rendering (photoreal, ray tracing)

Shown: path-traced macro glass, caustics, depth of field, and clay views.

- **Have:**
  - The PBR renderer (`crates/verse-pbr`, `crates/verse-engine/src/lighting.rs`)
    with shadows, sky and fog. Material transmission is in
    `crates/verse-engine/src/material.rs`.
  - An offline baker with GPU hardware ray queries (`crates/verse-bake/src/gpu.rs`).
  - Offline 1080p film renders with a keyframed camera rig, motion blur and
    depth of field (`crates/verse/examples/film_shot.rs`,
    `crates/verse-engine/src/camera_rig.rs`).
- **Partial:**
  - Glass is raster transmission. There are no caustics and no refraction of
    the scene behind except on water.
- **Missing:**
  - A progressive path tracer for hero frames.
  - A one-switch clay or debug render mode in film.

### Sensors (cameras, depth, tactile)

Shown: the force and velocity overlay. Camera views are implied.

- **Have:**
  - Physics sensors: raycasts, contact force, an IMU and debug lines
    (`crates/physics/src/sensors.rs`).
  - MuJoCo cameras in Robot Lab recordings.
  - Camera video in LeRobot datasets (`crates/robot-data`).
- **Partial:**
  - The in-zone Forces overlay exists, but film output deliberately carries no
    overlay (`crates/verse-film`).
- **Missing:**
  - Depth and segmentation images from the Verse renderer.
  - Tactile arrays.
  - Labelled force arrows in film.

### Locomotion and manipulation policies (RL)

Shown: text to reward to policy, epoch grids, sim-to-real.

- **Have:**
  - ACT and SmolVLA inference from pinned weights
    (`crates/robot-policy/src/act.rs`, `smolvla.rs`).
  - LeRobot dataset input and output (`crates/robot-data`).
- **Partial:**
  - Imitation learning only, and training is offline, outside the repo.
  - The current cube-placement score is 0/10 for both the policy and the
    scripted baseline.
- **Missing:**
  - RL training.
  - Reward authoring from text.
  - Parallel rollouts.
  - Legged robots.
  - A sim-to-real path. Robot Lab has a real SO-101 dataset path but no
    deployment loop.

### Character animation

Shown in the montage: a talking face, dancers and a performer.

- **Have:** skinned characters with layered animation graphs
  (`crates/verse-engine/src/animation_graph.rs`), clip libraries, and a
  character compiler (`crates/verse-content/src/compiler/characters.rs`).
- **Missing:** IK, retargeting, mocap, facial and blendshape animation, and
  physics-based characters.

### Destruction

Not a strong Genesis beat; the torn foam is the only one. It is our strength.

- **Have:**
  - The Meteor Showcase (`crates/verse/src/zones/meteor_showcase.rs`).
  - The CAD rover as 51 bodies with shear-breaking pins and a blast-wave model
    (`crates/verse/src/zones/meteor_rover.rs`, `crates/verse-zone-everglade/src/zones/everglade/cad_physics/blast.rs`).
  - The demolition support graph.
  - Lagrange 1 (`crates/verse-lagrange`, `docs/verse/lagrange-1.md`).
  - Lagrange 2's servoed arm factory fed by CAD mass properties
    (`crates/verse-lagrange2`, `docs/verse/lagrange-2.md`).
- **Missing:** runtime (Voronoi) fracture.

## Ranked gap list

Ranked by impact per unit of effort, smallest high-impact items first. Sizes:
S is days, M is one to two weeks, L is a month, XL is a quarter or more.

| Rank | Gap | Size | Unlocks shots | Notes |
|------|-----|------|---------------|-------|
| 1 | **Film annotation layer.** Opt-in force, velocity and contact arrows with live labels, drawn from the take's recorded physics telemetry | S | 7, 10–12 style; any robot or rover shot | `sensors.rs` already measures the quantities. Keep the default film frame clean; annotation is a separate layer in `crates/verse-film` |
| 2 | **Named camera directives on a take.** Orbit-descend, track-from-above, side-track and push-in as typed presets on `camera_rig`, rendered as a triptych | S | 5, 13 | Same take, several cameras; `verse-film` already records once and renders many times |
| 3 | **Clay and debug render mode** for film (grey material, collider wireframe, particle view) | S | 10–13 | Cheap and makes "under the hood" shots honest |
| 4 | **Convex-hull colliders**, then a static triangle-mesh collider for containers | M | 23, 18; better rover and debris | Biggest rigid-body hole. GJK/EPA or SAT on hulls; decompose meshes offline in the CAD and Blender pipelines |
| 5 | **XPBD cloth** extending `rope.rs` (stretch, bend, pin, collide with primitives) | M | 29, flags, sails, banners in zones | Reuses existing machinery; high visual value |
| 6 | **Typed `scene.v1` plus chat edits** (bodies, materials, timed forces, camera directives), run by `physics` and filmed by `verse-film` | M | 2, 5, 6, 8, 25–27 (an honest version of `gs.generate`) | Follows the Game Lab and CAD Lab pattern: JSON Patch, a check on every edit, semantic routing |
| 7 | **Legged and articulated robots in Robot Lab** through MuJoCo (quadruped MJCF/URDF, a dexterous hand) | M | 31–35, 38–40 | MuJoCo is already bound; this adds models, controllers and Verse drawing |
| 8 | **GPU particle liquid** (PBF or SPH in `wgpu` compute) with a screen-space fluid surface | L | 3, 19, 20, 22, 30 | First fluid; start with a bounded tank in the Water Lab |
| 9 | **Batched environments plus an RL loop** (vectorised MuJoCo or native, PPO, epoch-grid capture) | L | 32, 39 | Needs rank 7. A CPU batch is fine first; GPU comes later |
| 10 | **Depth and segmentation camera sensors** from the Verse renderer | S–M | Robot Lab data, policy inputs | Moved down because few shots need it, but it is cheap |
| 11 | **XPBD tetrahedral soft bodies** (and simple muscle actuation) | L | 17, 21, 26 | Tetrahedral meshing from the CAD and Blender pipelines |
| 12 | **Progressive path tracer** for hero frames, reusing the `verse-bake` BVH and ray queries | L | 3, 16–23 look | Offline only, 1080p film. Ray queries already exist |
| 13 | **MPM** (viscoplastic, granular, tearing solids) | XL | 16, 17 | Research-grade; consider after rank 8 |
| 14 | **Tactile sensors** | M | Manipulation | After rank 7 |
| 15 | **Runtime fracture** (Voronoi) | M | Destruction variety | We already fake it well with authored chunks |
| 16 | **Character IK, retargeting and facial animation** | L | Montage tiles | Separate from physics; lower priority for this reel |
| 17 | **Differentiable physics** | XL | None shown | Defer until there is a concrete optimisation use |
| 18 | **Sim-to-real loop** | XL | 36, 39, 40 | Needs hardware; SO-101 only at first |

## Verse demo ideas

These are original scenes that use what we have plus the cheapest gaps. They
are not copies of the reel's shots.

1. **"Measured grasp."** In Robot Lab, the SO-101 picks up a CAD Lab part (a
   bracket the chat just designed). The shot is filmed three ways at once:
   orbit, top-down tracking and wrist side-track (gap 2). On a second pass, the
   annotation layer (gap 1) labels grip force, gravity and slip velocity in
   newtons. Needs gaps 1 and 2 and the jaw-pad grasp that already works.
2. **"Rover under the shockwave."** The Meteor Showcase rover breaks up under
   a blast. A slow-motion replay freezes at peak load and labels each pin's
   shear force against its strength. The pins that fail turn red before they
   let go, then a clay pass shows the colliders (gaps 1 and 3). This is all
   existing physics, and it shows a strength Genesis did not.
3. **"Sketch to orbit."** A chat edits a part in CAD Lab. Its URDF drops into
   Robot Lab for a grasp test, and its mass feed goes to Lagrange 2, where the
   servoed arms install it on the keel and the test burn excites the keel mode.
   One cut shows a typed design passing through checked simulations at three
   scales. Needs nothing new; the film is a cut across existing zones.
4. **"Same task, different body."** Our honest answer to the embodiment
   sequence. One `scene.v1` task (move the block to the pad) runs on SO-101,
   Franka and a MuJoCo quadruped with a mounted arm, shown side by side, with
   success counts printed from real runs. Needs gaps 6 and 7.
5. **"Storm cargo."** A typed scene drops a pile of mixed CAD parts (hulls,
   gap 4) from a crane onto a barge riding FFT seas in the Water Lab. Buoyancy
   and drag already exist; the pile is the new part.
6. **"Flag on the station."** XPBD cloth (gap 5) as a banner on a Lagrange 1
   truss, flexing with the plume forces that already exist. It is a small first
   cloth demo that needs no gravity.

## Open questions for the repo study, answered

The repo study is [repo.md](repo.md). It pins `genesis-world` at
`df30b61a07c3bef3e8d53abbfa7f1c367ca77384` (v1.4.4). Each question is kept
with its answer.

1. **Licence:** what licence covers the physics engine, the renderer
   integrations and the bundled assets and robot models? Is it the same for
   `genesis-world` as for the older `Genesis` repo?
   - **Answer:** `genesis-world` is the old `Genesis` repo renamed; both URLs
     resolve to the same history. The engine and Quadrants are Apache-2.0,
     with no NOTICE file.
   - The rasteriser is a vendored pyrender. It is MIT upstream, but its notice
     was dropped.
   - LuisaRender is BSD-3, Madrona is MIT, and Nyx is a closed binary.
   - One vendored file (`ext/isaacgym/terrain_utils.py`) carries an NVIDIA
     proprietary header.
   - Assets are mixed. The Panda models are Apache-2.0, ANYmal C is BSD-3 and
     the UR5e is BSD. Go2, Shadow hand, KUKA and the meshes have no licence
     file.
2. **Generative interface:** did `gs.generate` (text to scene, camera, force
   and policy) ever ship? Or is it still the narrative in this reel, with only
   the physics engine public? What models drive it?
   - **Answer:** it never shipped. There is no `gs.generate`, no LLM client
     and no prompt handling anywhere in the repo.
3. **Architecture:** how are the solver families (rigid, MPM, SPH, FEM, PBD,
   stable fluid) coupled in one scene, and what decides the coupling order and
   time step?
   - **Answer:** the simulator steps solvers in a fixed order: tool, rigid,
     kinematic, MPM, SPH, PBD, FEM, SF.
   - All solvers share one substep rate, `dt / substeps`.
   - Each substep runs pre-coupling, then the coupler, then post-coupling.
   - There are three couplers:
     - legacy: grid- and particle-level pairs;
     - SAP: Drake-style, for rigid and implicit FEM;
     - IPC: through the external libuipc.
4. **Rigid solver:** is it maximal or reduced coordinates, what is the contact
   model (convex decomposition? SDF?), and how does it compare to MuJoCo on
   our SO-101 and Franka scenes?
   - **Answer:** reduced coordinates, ported from MuJoCo: CRBA, a sparse
     LDLᵀ, and the primal Newton or CG soft-constraint solver. Parity tests
     run against the `mujoco` library.
   - Contact uses MPR by default, or GJK/EPA. Meshes go through a CoACD convex
     decomposition. Non-convex pairs and terrain use a precomputed voxel SDF.
   - Since it tracks MuJoCo, our MuJoCo-backed `robot-sim` is already the
     closer comparison. We did not run SO-101 or Franka through it.
5. **GPU backend:** what is the compute layer (a Taichi-style kernel DSL or
   its successor), and is there a Metal/Apple-silicon path at useful speed? We
   ship on M-series Macs and phones.
   - **Answer:** Quadrants, a June 2025 fork of Taichi with about 393 kernels
     in Genesis.
   - Metal works, through SPIR-V and SPIRV-Cross, but it is 32-bit only: no
     `f64`, no 64-bit atomics, and no bounds checks.
   - There is no phone back end; iOS and Android were removed.
   - We did not measure speed.
6. **Benchmark method:** what does `hibernation` do, how many environments
   were active, and what are the numbers with contact-rich, non-idle scenes?
   - **Answer:** hibernation sleeps a whole constraint island after 10 steps
     below 2 mm/s with no applied force. It is experimental and off by
     default.
   - The headline number is the comment "43M FPS" in
     `examples/speed_benchmark/franka.py`. That is env-steps per second over
     30,000 Franka-on-a-plane environments. At `dt = 0.01` it equals
     "430,000x real time".
   - None of the checked-in benchmarks enable hibernation.
   - `tests/benchmarks/test_rigid.py` gives contact-rich scenes: Go2, G1
     falls, dexterous hands, table bussing and non-convex spacecraft. The
     metric is the same. Results go to a dashboard, not the repo.
7. **Rendering:** which ray tracer produced the beauty shots? Are they real
   time or offline, and how do they get from simulation particles to a liquid
   surface?
   - **Answer:** the ray tracer is LuisaRender, an offline-grade path tracer
     on CUDA, DirectX or Metal. It is now joined by the closed, CUDA-only Nyx
     plugin.
   - Particles become surfaces through splashsurf (Rust, MIT; the default) or
     the unlicensed OpenVDB-based ParticleMesher.
8. **Differentiability:** which solvers support gradients, and with what
   memory cost?
   - **Answer:** MPM, rigid, explicit FEM and tool only. PBD, SPH and SF are
     stubs.
   - Memory is one checkpoint of solver state per step, kept on the device;
     the backward pass replays substeps from it.
   - Rigid gradients force GJK and turn off hibernation and the elliptic cone.
9. **Assets:** where do the articulated object assets in the montage come
   from, and how are they licensed?
   - **Answer:** the repo ships one PartNet-Mobility object (`urdf/3763`) with
     no licence file. The montage's drill, laptop and toaster grid is not in
     the repo. Test robots such as the dexterous hands and G1 come from a
     Hugging Face dataset.
10. **Portability:** which pieces could we port into `crates/physics`
    (CPU-first, deterministic, no zone types), and which would only ever be an
    offline oracle like `crates/physics/oracle/genesis_oracle.py`?
    - **Answer:** port as algorithm reimplementations:
      - convex-hull narrow phase;
      - static SDF colliders, with an offline CoACD bake;
      - XPBD cloth;
      - PBF liquid, CPU first and then wgpu;
      - batched environments;
      - DLS IK;
      - tactile taxels.
    - Keep as oracle or reference only: the MuJoCo-port rigid solver (we call
      MuJoCo), MPM, FEM, the SAP and IPC couplers, differentiability, and every
      renderer.
    - See the ranked plan in [repo.md](repo.md).
