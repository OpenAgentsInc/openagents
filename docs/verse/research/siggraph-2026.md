# SIGGRAPH 2026 papers for Verse

This note ranks SIGGRAPH 2026 papers, talks, and a few 2026 papers from
related venues by their value to Verse, Everglade, water, characters, and
agents. It was compiled on 2026-10-07. It links each paper and doesn't
summarize beyond one line.

## Sources

- [Ke-Sen Huang's SIGGRAPH 2026 page](https://kesen.realtimerendering.com/sig2026.html).
  On 2026-10-07 it filled in about 125 papers. The remaining sessions are
  placeholders, so the full session list came from the
  [official schedule](https://s2026.conference-schedule.org/?filter1=sstype132)
  (309 technical papers).
- [Advances in Real-Time Rendering 2026](https://advances.realtimerendering.com/s2026/index.html).
- [Self Shadow's SIGGRAPH 2026 links](https://blog.selfshadow.com/2026/07/26/siggraph-2026-links/)
  for courses, talks, and posters.
- arXiv and project pages for the papers themselves, and Crossref for
  metadata. The ACM Digital Library refuses automated fetches.

## Constraints that set feasibility

- Every Verse backend runs under WebGL2 limits today, so there are no
  compute shaders or storage buffers on any tier ([water.md](../water.md),
  "Simulation: CPU everywhere, GPU where it pays"). A technique that needs
  compute is a High-tier option on desktop wgpu (Metal) only, and that path
  is an open owner decision.
- There's no hardware ray tracing and no CUDA. Anything that needs either is
  a "watch", not a "do".
- Characters use the Universal rig's 65 joints (71 with optional leaves), and
  `retarget_clip` maps clips by joint name
  ([female-character.md](../female-character.md#skeleton)).
- Villagers and Alice speak through text bubbles today. Nothing in Verse
  synthesizes speech audio yet.

Effort uses three sizes: **S** is a few days, **M** is one to two weeks, and
**L** is a month or more.

## Do these first

Ranked by value over effort.

1. **[Physics-Inspired Procedural Texturing of Extremely Deformable Surfaces](https://visualcomputing.ist.ac.at/publications/2026/DeformableTextures/)**
   (ISTA). A fragment-shader texture that stays
   undistorted under extreme stretching, with no simulation and open
   [code](https://git.ista.ac.at/wojtan-group/aleksei-kalinov/stdf_wavetextures).
   It fits foam on flowing water, Control Water, lava, and mud spells on
   every tier, including WebGL2. Effort: **S**.
2. **[LightOpt: Lights Optimization for Real-time Rendering](https://mirage-c.github.io/LightOpt/)**.
   An offline pass removes, merges, and inserts lights so that a scene looks
   the same with 20 to 53 percent fewer lights. Everglade's lanterns and
   windows at night are a phone cost, and this runs in the pack build, not at
   run time. We'd reimplement the loss against our own renderer's captures.
   Effort: **M**.
3. **[Stylized Text-to-Motion via Hypernetwork-Driven LoRA](https://junhyukjeon.github.io/projects/style-salad/)**
   ([code](https://github.com/junhyukjeon/style-salad)). Generates text-driven
   motion in a named style. Run it offline to bake per-villager idle, walk, and
   work clips ("tired baker", "proud guard"), then retarget them to the
   Universal rig. Nothing ships but clips. Effort: **M**.
4. **[SLIM: Scaling User-Generated 3D Worlds on Roblox](https://advances.realtimerendering.com/s2026/content/SMAK_SIG26_SLIM-Scaling_User-Generated_3D_Worlds_on_Roblox_8_Aug_2026.pdf)**
   (Advances). Kit-bashed worlds get automatic, device-adaptive runtime
   representations, with a fallback to the original pieces when behavior
   needs them. That's the shape of Everglade's kit houses on web and phone
   (refactor phase P8) and of HLOD in [rendering-scale.md](../rendering-scale.md).
   A carved building falls back to its pieces. Effort: **M** for the design;
   the build work is already in the P8 plan.
5. **Streaming co-speech gesture on the host** (see
   [Gesture for Alice and the villagers](#gesture-for-alice-and-the-villagers)).
   Start with a distilled, causal model in the style of GestureFAR on Apple
   Silicon, driving the upper body of the Universal rig. It's the largest item
   on this list, and the one the owner asked about. Effort: **L**.

## All picks by area

### Water and fluids

| Paper | What it does | Why it matters | Stack fit | Effort |
| --- | --- | --- | --- | --- |
| [Physics-Inspired Procedural Texturing](https://visualcomputing.ist.ac.at/publications/2026/DeformableTextures/) | Procedural textures on extremely deforming surfaces | Water foam and flow, lava, spells on water | Fragment shader; web and phone | S |
| [Real-Time Interactive Hybrid Ocean](https://arxiv.org/abs/2511.02852) (2025 preprint, not SIGGRAPH) | Wave-particle patches around objects, coupled to an FFT ocean with one spectrum | Wakes and ripples around rowboats that match the FFT sea instead of a separate ripple grid | CPU patches plus our CPU FFT; web and phone | M |
| [Spatiotemporal FLIP](https://ge.in.tum.de/download/ST-FLIP.pdf) (honorable mention) | Jitters FLIP particles in time for steps up to ten times larger | Offline splash and waterfall bakes for flipbooks; not run time | Offline CPU | M |
| [Learning Surfing-like Balance without Water Simulation](https://cgrhyu.github.io/publications/2026-learning-surfing-like.html) (poster) | A staged RL policy balances on a moving board without fluid simulation | Standing in a rowboat that rocks with buoyancy | Small policy on CPU; training offline | M |
| [MPM Lite](https://arxiv.org/abs/2602.07853) | MPM with linear kernels and no particle quadrature at solve time | Mud, sand, and snow for spells; a later Genesis port step | GPU compute or offline; not WebGL2 | L |

### Rendering, lighting, and volumes

| Paper | What it does | Why it matters | Stack fit | Effort |
| --- | --- | --- | --- | --- |
| [LightOpt](https://mirage-c.github.io/LightOpt/) | Offline light-count reduction that keeps the image | Everglade night lighting on phones | Build time only | M |
| [Smolder](https://advances.realtimerendering.com/s2026/content/AlexanderMueller_Smolder_Siggraph26.pdf) (Advances, IO Interactive) | Volumetric effects used like any other VFX, lit by the scene | Fireball smoke, fog spells, weather | Assume compute; needs a ray-marched fragment fallback for WebGL2 | L |
| [Adaptive Tessellation and Subdivision](https://advances.realtimerendering.com/s2026/content/Adaptive%20Tessellation%20and%20Subdivision_7.pdf) (Advances, Meta) | Clamped parallelogram tessellation pattern, screen-adaptive with welded seams | The pattern applies to water grids and terrain even when built on the CPU | Compute in the talk; CPU pattern is portable | M |
| [Gabor Fields](https://arcanous98.github.io/projectPages/gaborVolumes.html) | Orientation-selective LOD for volumes | Distant clouds and smoke at a fixed cost | Research code; ray-march in a fragment shader | M |
| [Multi-feature Radiance Baking Neural Networks](https://researchr.org/publication/LiangYGWL26/bibtex) | Baked neural radiance for instant volume rendering | Clouds and weather lighting | Small MLP per sample; heavy on phones | L |
| [Upgrading PSSR](https://advances.realtimerendering.com/s2026/content/siggraph2026_advances_dcraig_v1_0_publish.pdf) (Advances, Sony) | Puts closed-form steps back and gives the ML upscaler less to do | Guidance if phones ever get an upscaler | Lessons, not code | S to read |
| [Real-Time LOD Rendering with ReSTIR](https://research.nvidia.com/labs/rtr/publication/wang2026levelofdetail/) | Sample reuse across LOD switches | Watch only | Needs ray tracing | — |

### Geometry, textures, and streaming

| Paper | What it does | Why it matters | Stack fit | Effort |
| --- | --- | --- | --- | --- |
| [SLIM](https://advances.realtimerendering.com/s2026/content/SMAK_SIG26_SLIM-Scaling_User-Generated_3D_Worlds_on_Roblox_8_Aug_2026.pdf) (Advances, Roblox) | Device-adaptive representations of kit-built worlds | Everglade on web and phone, HLOD, carved-building fallback | Offline pipeline | M |
| [Neural Texture Compression using Hypernetworks](https://arxiv.org/abs/2606.26913) (EGSR 2026) | One hypernetwork outputs per-material latents and decoder weights | Smaller packs for Everglade's materials | MLP decode per texel in shading; desktop only for now | L |
| [Taming Optimization Variance in Compact Neural Shading](https://research.nvidia.com/labs/rtr/publication/bitterli2026taming/) | Ensemble training that makes tiny shading networks converge reliably | Read before training any neural material or compressor | Training side only | S to read |
| [Robust In-Engine Texture Optimization](https://ksp.etri.re.kr/ksp/article/read?id=72986) | Fits textures to inconsistent AI-generated views inside a non-differentiable rasterizer | Generated textures for kit pieces in the Blender pipeline | Offline; finite differences in our renderer | M |
| [Neural Shading course](https://github.com/shader-slang/neural-shading-s26) and [Moving Mobile Graphics](https://developer.arm.com/community/arm-community-blogs/b/mobile-graphics-and-gaming-blog/posts/moving-mobile-graphics) (courses) | Introductions to neural shading in Slang, and mobile GPU practice | Phone budgets and any neural material work | Slang can emit WGSL | S to read |

### Destruction and physics

| Paper | What it does | Why it matters | Stack fit | Effort |
| --- | --- | --- | --- | --- |
| [M-ABD](https://arxiv.org/abs/2603.08079) | Multi-affine-body dynamics with constant system matrices for jointed assemblies | Collapsing kit buildings after the support graph breaks; Genesis port articulated scenes | CPU-viable at small counts; deterministic | L |
| [Progressing LOD Animation of Volumetric Elastodynamics](https://arxiv.org/abs/2509.14177) | Coarse previews that predict a fine elastic simulation | Authoring-time previews for physics scenes | Offline | M |

### Characters and motion

| Paper | What it does | Why it matters | Stack fit | Effort |
| --- | --- | --- | --- | --- |
| [Stylized Text-to-Motion LoRA](https://junhyukjeon.github.io/projects/style-salad/) | Text-driven motion with a named style | Baked per-villager clip variety | Offline; clips ship | M |
| [Skinned Motion Retargeting with Interaction Guidance](https://vml.kaist.ac.kr/assets/Contents/Publications/International/2026SoojinChoi_SIGGRAPH/2026SoojinChoi_SIGGRAPH.html) | Retargeting that keeps self-contact on different body shapes | Clips on Alice's and villagers' stylized proportions without hands passing through bodies | Offline | M |
| [MotionBricks](https://nvlabs.github.io/motionbricks/) | One latent model with 350,000 skills, about 2 ms latency, smart primitives | Villager locomotion and object interaction from goals | GPU on the host; trained on a robot skeleton, so retargeting needed | L |
| [ARDY](https://research.nvidia.com/labs/sil/projects/ardy/) | Streaming motion from online text and kinematic constraints | Alice acting on typed commands | Host GPU | L |
| [Secrets of the Animal Kingdom: Crowds in Hoppers](https://research.pixar.com/#pub-2026-siggraphtalks-nnl) and [Scalable Stylized Fire](https://research.pixar.com/#pub-2026-siggraphtalks-ggl) (Pixar talks) | Production crowd and stylized fire methods | Market-day crowds and fire spells in our art style | Ideas only | S to read |

### Agents, audio, and speech

| Paper | What it does | Why it matters | Stack fit | Effort |
| --- | --- | --- | --- | --- |
| [EchoAvatar](https://robinwitch.github.io/EchoAvatar-Page/) ([arXiv](https://arxiv.org/abs/2605.28272)) | Full-body motion from streaming speech or music, retargeted to one character | Gesture while Alice talks; dance at the inn | Host GPU; open code | L |
| [Reality Check](https://arxiv.org/abs/2605.06063) | How avatar and face rendering bias gesture ratings | Design of any gesture evaluation on our stylized characters | Study only | S to read |
| [Reciprocal Latent Fields](https://arxiv.org/abs/2602.06937) | Compact latent grid for precomputed sound propagation with reciprocity | Street and tavern acoustics in Everglade | Offline bake; small CPU decode at run time | M |

## Gesture for Alice and the villagers

### The owner's paper

[Causal Temporal Padding for Low-Latency Real-Time Gesture Generation](https://doi.org/10.1145/3776574.3831207)
by Ryo Ishii, Shinichiro Eitoku, Jiro Nagao, and Junichi Sawase appears in
the Proceedings of ICMI 2026 (published 2026-10-04, per Crossref). We found
no open preprint, and Crossref has no abstract. From the title, it trains a
gesture model with padding only on the past side, so that it needs no future
audio and its latency is bounded by its chunk. That's the same causal
encoder idea LiveGesture uses. Read the paper itself before citing details.

### Related open streaming work

| Paper | Latency and size | Representation | Notes |
| --- | --- | --- | --- |
| [GestureFAR](https://arxiv.org/abs/2609.21576) (arXiv, 2026-09) | 9.3 ms per token after one-step distillation; 0.246 real-time factor on a MacBook Air; 6.51 M-parameter flow head over a frozen mHuBERT encoder | Causal VAE latents at 7.5 Hz, rendered at 30 FPS | Closest to on-device; no lookahead |
| [LiveGesture](https://arxiv.org/abs/2604.10927) ([CVPR 2026](https://openaccess.thecvf.com/content/CVPR2026/papers/Saleem_LiveGesture_Streamable_Co-Speech_Gesture_Generation_Model_CVPR_2026_paper.pdf)) | Under 50 ms per 200 ms chunk; 0.5 M-parameter causal audio encoder | SMPL-X regions in Rot6D: upper body, hands, lower body, and FLAME face | Zero lookahead, region experts |
| [EchoAvatar](https://arxiv.org/abs/2605.28272) (SIGGRAPH 2026) | Compute stays well under a 266 ms audio chunk on an RTX 4090 | Root velocity, height, and 6D rotations retargeted to one character | Open code and weights; speech and music |
| [MIBURI](https://openaccess.thecvf.com/content/CVPR2026/papers/Mughal_MIBURI_Towards_Expressive_Interactive_Gesture_Synthesis_CVPR_2026_paper.pdf) (CVPR 2026) | Interactive gesture synthesis | — | A second CVPR 2026 reference point |

### Is it viable?

Yes, on the host, for the upper body. Here's why, and what it costs:

- **Audio first.** All of these models take speech audio. Verse NPCs speak
  in text, so gesture depends on adding text-to-speech. Without speech
  audio, use timed beat gestures from the text and the existing clips.
- **Villagers don't need streaming.** A villager's line is complete before it
  appears, so the host can synthesize the speech and generate the whole
  gesture clip before playback starts. Latency is one clip's generation time,
  not a chunk. Streaming and causal padding matter for Alice, whose replies
  stream from a model.
- **Where it runs.** GestureFAR's numbers put a distilled causal model at four
  times real time on a laptop CPU or GPU, so the Coder host on Apple Silicon
  can run it for a handful of speakers. Phones are marginal, mostly because of
  the speech encoder (mHuBERT is around 95 M parameters), and the web is out
  for now. Clients receive keyframes, not a model.
- **Driving our skeleton.** Map SMPL-X body and hand joints to the Universal
  rig by name and correct rest poses, the same way `retarget_clip` corrects
  them. Keep the upper body and hands, and blend them over the locomotion
  layer. Drop the FLAME face; drive the optional `jaw` joint from speech
  energy. About 20 joints at 15 Hz, quantized, is small enough to attach to
  the talk event rather than to NIP-MV pose frames.
- **Data.** These models train on BEAT2 or ZEGGS. As far as we can tell, both
  restrict commercial use. Confirm the licenses before shipping weights, or
  plan to record our own data.
- **Evaluation.** Reality Check shows that avatar and face rendering bias
  gesture ratings, so judge gestures on our own stylized characters.

Suggested order: beat gestures from text timing (S); then speech synthesis
plus an EchoAvatar or GestureFAR model on the host for villagers, generating
whole clips (M to L); then streaming for Alice with causal chunks (L).
