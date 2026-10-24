# 0011: Rendering and shaders

Status: Accepted. Restates Aethyrforge 0007 and 0008, written on 2026-10-24 to
the owner's direction ("the best low-level stack").

## Decision

- **GPU-driven rendering on every platform:** scene tables live on the GPU.
  Compute passes do frustum and two-phase occlusion culling and write
  indirect draws. Opaque geometry uses a visibility buffer. Temporal
  anti-aliasing with upscaling is the default, with MSAA as the fallback.
  Dense static geometry uses cluster LOD, and crowds use tiers (skinned near,
  vertex-animation-texture middle, impostor far). Capability-selected fast
  paths have baseline fallbacks. A technique stays only if it measurably
  reduces GPU time on pinned profiles.
- **Shaders are written in Slang and shipped as WGSL.** Engine shaders are
  compiled offline to WGSL, validated by both Tint and Naga. One WGSL artifact
  serves every platform. Creator materials stay WGSL through the material
  hook.
- **The render lab is the proving ground.** Each technique is measured there
  on real GPUs before it ships, on web and native. Headless Linux Chrome
  falls back to a CPU rasterizer, so it is not evidence.
