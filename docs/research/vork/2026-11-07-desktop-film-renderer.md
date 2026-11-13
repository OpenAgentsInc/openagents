# Desktop film rendering for the Metaverse Engine

Research note, 2026-11-07. Owner question: we want very high-quality
renders of Everglade and other worlds to make movies. Can we, and should
we, have a desktop rendering flow like vork's instead of doing everything
in the browser, while reusing as much of the engine as possible?

**Recommendation: yes. Build it as a `film` render profile on the OpenAgents Metaverse Engine's
existing native Vulkan and Metal hosts, not as a separate renderer and not
on vork.** It should first use what we already have (determinism,
replayable sessions, Coax shaders emitted as SPIR-V and Metal, the
headless `vk-realm`), then add hardware ray queries. Path tracing is not
worth building.

## Where movies come from today

Every movie we make is a screenshot of the real-time frame.

| Path | How it works | Ceiling |
| --- | --- | --- |
| Gameplay clips (`coax/web/clip.coax`, `docs/metaverse/clips.md`) | Headless Chrome with WebGPU; `host.js` clip mode steps one fixed game step a frame; PNGs; ffmpeg | The live WebGPU shaders at the live quality rung, 8-bit PNG |
| Animation Lab films (`metaverse-engine/tools/anim-scene-play.coax`, `docs/metaverse/anim-scenes-v1.md`) | Headless muted Chrome over CDP at 960×540; `animFrame(frame,fps)` then a screenshot; per-frame RGBA hashes; ffmpeg H.264 and AAC; at most 60 s a call | Neutral inspection shading (Lambert plus ambient, no shadows); 1.62 s per film second (A5) |
| Echo in Everglade (`coax/web/everglade-echo-frames.coax`) | 1920×1080 at 24 fps from the live page | Live pass; the 60 FPS gate is currently missed (#12984) |
| Raid promo (`?raid=preview&cine=1`, `apps/openagents-web/everglade-raid-preview.coax`) | Keyed camera flight, captured as a clip | Same as gameplay clips |

The determinism these give is already valuable: Animation Lab's A5
rendered the same film twice and got 720 identical frame hashes and the
same MP4 hash. What they can't give is any quality the real-time frame
doesn't have. Our frames are built under a 16.7 ms budget, and on Everglade
that means:

- LOD at 52 to 72 m, real trees only within 55 m, and cones past that.
- A 60 m shadow reach, a 2048² map and 9 PCF taps.
- No blending in the scene pass, so transparency is screen-door dithered.
- Screen-space water reflections with 20 steps.
- The adaptive quality ladder.

None of those limits apply to a film frame.

## What we already have natively

This is why a desktop flow is cheap for us, and why it should reuse the
engine.

- **All 16 realms run natively on Vulkan** (release `2a94f5fab2`,
  2026-10-31; `docs/metaverse/native-gpu.md`), with every 90th frame identical
  to `vk-passes` and Everglade at 60 FPS on the RTX 4080.
- **Coax writes the SPIR-V.** `coax/lib/compiler/spirv.coax` lowers the
  checked tree, and the C++ driver only writes the words. The same Coax
  procedures emit WGSL for the web build (G6). `coax metal --vertex
  --fragment` emits complete Metal stages; all eleven Everglade entry pairs
  compile, and the Mac host runs Everglade at 120 Hz (#12867, #12868).
- **A headless native render already exists.** `metaverse/gpu/vk-realm.c`
  takes `--headless FRAMES [--out OUT.f32]`: it steps the realm at 1/60 s,
  draws every frame and writes the last frame's targets as float RGBA. Its
  graph target is `R16G16B16A16_SFLOAT`. `metaverse/gpu/realm.sh HOST check`
  already times frames at 320×180, 2560×1440 and 3840×2160. Headless key
  input (`F:KEY:DOWN,...`) is there too.
- **The worlds replay exactly.** `metaverse-engine/core/loop.coax` has one clock with a
  fixed step. `metaverse-engine/core/replay.coax` records every accepted external event
  and a digest every step. `metaverse-engine/core/rand.coax` draws are a pure function of
  seed, stream and counter. Puff effects are a pure function of seed, index
  and age (`metaverse-engine/fx/puff.coax`). Animation Lab takes and cuts are typed
  documents (`anim.project.v1`, ALP1).
- **A CPU reference renderer exists** (`metaverse-engine/render/reference-*.coax`),
  with bounded offline work profiles (`reference-work.coax`) and an image
  comparison rule from chapter 11.

vork has to build all of this itself. For example, its scene simulation
replays from the start or from a checkpoint to seek. We have it already.

## What to take from vork's design

1. **Scene content and render quality are separate.** A vork profile
   (`std/previs`, `std/draft`, `std/final`, `std/path`) changes sampling,
   transport and denoising, never the scene. Camera optics and motion blur
   belong to the scene. For us: a realm state, staged moment, take or cut
   is the content, and `live` or `film` is the profile. The film profile
   calls the same Coax shading functions (`eg_paint`, `cc_player_light`,
   the character pass) and changes only their visibility and sampling
   inputs: the shadow term, AO, reflection, sample positions and time.
   Everglade stays painterly under a film render.
2. **Renders are many samples of time and lens, averaged.** vork has
   `render.motion_blur` (`shutter` in frames, `position`
   start/center/end), physical camera inputs that follow Blender (`lens`,
   `sensor_width`, `focus_distance`, `aperture_*`, `focus_object`), and a
   `seed`. Because our clock is fixed-step and replayable, motion blur is
   N evaluations of the world at sub-step times inside the shutter, and
   depth of field is N aperture positions. Each one is an ordinary
   deterministic frame.
3. **Outputs carry their own metadata.** vork writes:
   - multilayer OpenEXR, scene-linear and premultiplied;
   - channels for depth (metres), view normal, object index, motion
     vectors and shadow catcher;
   - an `OUT.meta.json` per output with fps, per-frame camera, depth unit
     and an object-id-to-node map;
   - an `OUT.camera.json` with OpenCV intrinsics and extrinsics per frame;
   - ProRes 4444 with alpha.
   
   That is what lets you composite, grade, retime and add titles without
   re-rendering. Our `labs cut` and `anim.cut.v1` tooling would use it.
4. **Previews for the agent loop.** `vork render shot.json --profile
   std/previs --at 3 --set resolution=320x180 --out still.png` gives one
   small still at a chosen time. Animation Lab's directing loop (A7) and
   the Metaverse Engine's edits could show the model such a frame after each change.
5. **Honest timing.** vork reports setup, warmup and first-compile time
   separately, marks unmeasured comparisons as "unmeasured, not wins", and
   labels projections as projections. Chapter 37's draft rule (compile and
   startup time count) is stricter. Keep it, and borrow the table layout.

## What not to take

- **Slang, or any outside shader toolchain.** `docs/metaverse/native-gpu.md`
  rules out glslang, naga, tint, Dawn and wgpu, and the same reasoning
  covers Slang. Our film shaders are Coax procedures with `#test` checks,
  emitted as SPIR-V and Metal.
- **vork's code.** Its licence forbids distributing modified copies and
  hosted rendering. Read it for ideas only.
- **Python preparation pipelines.** vork's VFX example uses OpenCV, SAM 2.1
  and Depth Anything in Python. Our tools are Coax (`AGENTS.md`), and model
  work belongs in Psionic.
- **Path tracing as a goal.** vork's own path tracer is 2–6× slower than
  Cycles on its benchmark scenes (README, "Path tracing"). It is a
  research project that doesn't fit stylized worlds. Painterly and cel
  looks gain from better visibility and sampling, not from unbiased light
  transport.
- **Waiting on vork for volumes.** vork has no fog or participating media
  in 0.1.0. Everglade already has height haze, rain, light shafts and
  underwater shafts.

## The design

```
realm Coax (unchanged)  ──▶  state at film time t (replay / stage / take / cut)
                                   │
                    film profile:  │  for each frame: for each sample s
                                   │    t_s  = shutter sample (sub-step of the fixed clock)
                                   │    lens = aperture sample, jitter = pixel sample
                                   │    draw the realm's own passes (same Coax shaders)
                                   │      with film visibility (bigger maps / ray queries)
                                   ▼
   accumulate f32 linear ──▶ EXR layers + AOVs + meta.json + camera.json
                                   │
                         grade/encode (H.264 review, ProRes/EXR master)
```

### The `film` profile

This is a Coax record next to `metaverse-engine/core/quality.coax`. It is not a new rung
on the ladder, because the ladder exists to hold 60 FPS and a film frame
has no frame budget. A film budget still refuses, per chapter 16, in
memory, draw records and seconds per frame, so a batch fails early instead
of running out of GPU memory at frame 3,000. It sets:

- **Samples**: pixel jitter, N per pixel, as a stratified pattern from
  `metaverse-engine/core/rand.coax` streams keyed by frame and sample.
- **Shutter**: frames and position, as in vork. A sample's time is a
  fraction of the fixed step. The renderer interpolates drawn state with
  `loop.coax`'s existing interpolation, and simulation is never stepped
  differently for a render.
- **Lens**: aperture, focus distance or a focus entity. The camera record
  in `metaverse-engine/cam/shot.coax`, `metaverse-engine/cam/cinematic.coax` and Animation Lab's scene
  cameras gains physical fields (lens, sensor, focus, f-stop). Today
  Animation Lab stores `focus` but nothing reads it.
- **Detail**: LOD distances to infinity (or a stated far plane), real kit
  trees everywhere, full grass density, full puff and debris budgets,
  shadow cascades instead of the 60 m follow map, and full-rate far-figure
  animation.
- **Transport**: `raster` (phase 1) or `traced` (phase 2), choosing where
  shadow, AO and reflection terms come from.
- **Outputs**: EXR channels and sidecars.

### Phases

Each phase has an exit check that runs headless on our own machines, as
`native-gpu.md` requires.

| Phase | What | Exit check |
| --- | --- | --- |
| **F0. Native film frames** | `vk-realm` (and the Metal host) render a frame sequence from a stage, a replay record or an ALP1 cut. They write per-frame f32 or EXR plus `meta.json`, and the film path replaces Chrome capture for native realms. Audio comes from the same edit clock, as now. | At one sample with the film profile set to live settings, the frames match the current headless-Chrome goldens within chapter 11's colour rule. Two runs give identical frame hashes. |
| **F1. Accumulation** | Pixel jitter, shutter sub-steps and aperture samples are averaged in f32. The film profile lifts LOD, quality and budgets. | Motion blur against an analytic moving quad. Depth of field against the thin-lens circle of confusion. Variance falls as 1/N on a fixed scene. The film profile changes no simulation digest. |
| **F2. Film-only passes** | Sorted or weighted-blended transparency instead of screen-door. Cascaded shadows. Higher-sample water reflections and light shafts. AOVs: depth, normal, object id, motion. `camera.json`. | Each new pass has a CPU reference case in `metaverse-engine/render/reference-*`. Every channel round-trips through the EXR writer, which is written in Coax. |
| **F3. Hardware ray queries** | `coax spirv` learns `SPV_KHR_ray_query`. The C host builds bottom- and top-level acceleration structures (`VK_KHR_acceleration_structure`), which is a platform effect like any other buffer; Metal uses `MTLAccelerationStructure`. Use them for area-light and sun shadows, AO, water and window reflections, and optionally one diffuse bounce, all feeding the existing shading terms. | Ray visibility equals a CPU ray cast over the same triangles (`metaverse-engine/physics/query.coax`, `ground-query.coax`) on fixtures. A furnace-style energy check on a closed box. Validation layers report 0/0. |
| **F4. Denoising** (optional) | Only needed when traced samples are few. Our own denoiser in Coax, run by Psionic (owner, 2026-11-07). OIDN and others serve only as comparison references in opt-in harnesses. | Measured against a high-sample reference on fixed scenes, with the error reported, not asserted, and compared with OIDN on the same inputs. |
| **F5. GPU simulation for crowds** (separate) | Seedable, seekable compute for flocks, crowds and particles at film scale, in Coax compute. | A seek to frame k equals stepping from S0 (the existing replay digest rule). |

F0 to F2 need no new GPU features and should already make Everglade films
look far better than live capture. F3 is a compiler and host project on
the scale of G1 or G2. F4 and F5 should wait until a film needs them.

### Hardware

- The **RTX 4080** (`coderos-4080`) has hardware ray tracing. It is the
  right render box, and it already runs every realm natively.
- **Apple Silicon** has hardware ray tracing from M3. M1 and M2 Macs can
  run F0 to F2 but not F3, so the film profile has to refuse `traced` on
  them rather than silently fall back.
- For **cloud rendering**, vork's note applies: A100, H100, H200, B200 and
  B300 lack the hardware ray tracing that F3 needs, while L4, L40S and RTX
  cards have it. Cloud Run GPU shards would need an RT-capable card for
  traced films.

### Reuse

| Engine piece | Film use |
| --- | --- |
| `metaverse-engine/core/loop.coax`, `replay.coax`, `rand.coax` | Exact replay at film time; sample streams |
| Realm Coax and its stages (`?stage=`, `?raid=preview&cine=1`) | Unchanged scene content |
| `apps/openagents-web/gpu/everglade-v1/gpu.coax`, `gpu-fx.coax`, `metaverse-engine/render/character.coax`, `apps/openagents-web/gpu/*.coax` | The same shading functions, with film visibility inputs |
| `coax spirv`, `coax metal`, `metaverse/gpu/vk-*.c` | The native pipeline and host, extended |
| `metaverse-engine/render/passes.coax`, `catalog.coax` | The pass graph as data; the film profile selects extra passes |
| `metaverse-engine/render/reference-*.coax` | CPU oracles for each new pass |
| Animation Lab ALP1, `anim.cut.v1`, `labs anim play` | The edit, cameras and audio clock for films |
| `metaverse-engine/audio`, `metaverse-engine/audio/speak.coax` | Score, voices and the mix, already offline and deterministic |

## Risks

- **Two looks drifting apart.** If film shading forks from live shading,
  film frames stop matching the game. Mitigation: the F0 check (film at
  live settings equals live) stays in the suite, and film-only code is
  limited to visibility and sampling.
- **Ray-query work in the compiler.** It adds new SPIR-V types, a
  capability, and acceleration-structure bindings in the resource ledger
  (`metaverse-engine/render/registry.coax`). It is bounded, but it touches the
  compiler's mutation gate.
- **Asset quality becomes the limit.** At film quality, Quaternius
  low-poly kits, 512px wardrobe textures and baked AO grids will show.
  The film profile shows the art as it is; it doesn't improve it.
- **Echo-class characters** are already over budget at about 3.13 GB of
  GPU textures. That is fine for film and is a reason to keep film and
  live budgets separate.

## Decisions

- **2026-11-07, owner: film mode is approved.** Build the `film` profile on
  the native hosts, starting with F0 to F2, with `coderos-4080` as the
  render box.
- **2026-11-07, owner: we write our own denoiser when one is needed.** F4 is
  a denoiser in Coax, run by Psionic. Intel OIDN and other denoisers are
  allowed only as comparison references in opt-in benchmark and parity
  harnesses, the way cuBLAS is used for Psionic. They are never linked into
  anything shipped. No OIDN exception is requested.
- **Open:** F3, ray-query support in `coax spirv` and acceleration
  structures in the C host, is decided when F0 to F2 are done.
