# vork: what it teaches the Metaverse Engine

Research notes, 2026-11-07. Read from a shallow clone of
[voyager-so/vork](https://github.com/voyager-so/vork) at `44188ba` (release
0.1.0, research preview), announced by Ravi Parikh on 2026-11-06.

vork is a headless 3D and VFX renderer built for agents. An agent writes a
JSON scene (plus optional Slang programs), renders a cheap preview, looks at
the image, revises, and renders at final quality. It is Rust and Slang over
Vulkan and Metal, uses hardware ray tracing on NVIDIA and Apple GPUs, and
denoises with bundled Intel OIDN. Its headline is a million simulated birds
rendered on one NVIDIA L4.

| Document | What it covers |
| --- | --- |
| [Desktop film rendering for the Metaverse Engine](2026-11-07-desktop-film-renderer.md) | Whether and how to add an offline "film" render path to the OpenAgents Metaverse Engine's native hosts for Everglade, Animation Lab and Character Create movies, reusing the engine rather than adopting vork |
| [vork against Animation Lab, Everglade and Character Create](2026-11-07-vork-and-our-realms.md) | Feature-by-feature: what vork has, what each of our three surfaces has today, and what is worth taking |

## The short answer

Owner decision, 2026-11-07: film mode is approved (see the film note's
*Decisions* section).


- **Build a desktop film renderer, but as a profile of the Metaverse Engine, not a second
  engine.** All 16 realms already run natively on Vulkan (G0–G6,
  `docs/metaverse/native-gpu.md`), the Mac Metal host runs Everglade at 120 Hz,
  and `metaverse/gpu/vk-realm.c --headless` already writes float frames. Every
  movie today is a screenshot of the real-time WebGPU frame in headless
  Chrome (`coax/web/clip.coax`, `metaverse-engine/tools/anim-scene-play.coax`). The
  biggest quality jump is from removing that ceiling, not from ray tracing.
- **The idea worth copying most is vork's split between scene content and
  render quality.** One scene, several named profiles (`std/previs`,
  `std/draft`, `std/final`, `std/path`). For us: one realm state, take or
  cut, rendered `live` or `film`, with the same Coax shading functions.
- **Phase 1 needs no ray tracing**, because our worlds are deterministic and
  replayable. A film frame can be rendered many times and averaged:
  supersampled, motion-blurred across sub-steps of the fixed clock,
  defocused by aperture jitter, with LOD and the quality ladder switched
  off. That converges without a denoiser.
- **Hardware ray queries come second**, for shadows, AO, water reflections
  and one-bounce light. That needs ray-query support in our Coax SPIR-V
  writer and acceleration structures in the C host. It is real work, but it
  fits the existing rules.
- **Do not adopt vork itself.** Its licence forbids redistributing modified
  copies and hosted use. Slang is an outside shader toolchain, which
  `docs/metaverse/native-gpu.md` rules out. Its preparation pipelines are
  Python. Read it for design, don't copy its code.

## Licence notes

- vork: "vork Preview License", source-available, free during the preview
  including for commercial work. No redistributing modified copies, no
  hosted rendering service, and this version only.
- `integrations/blender/` in vork is GPL-3.0-or-later and separate.
- The bundled OIDN denoiser is Apache-2.0 (Intel). Owner decision,
  2026-11-07: we write our own denoiser, and OIDN is used only as a
  comparison reference in opt-in harnesses.
