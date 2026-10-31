# 182. Pictures for policies

> **Status: unimplemented — design draft.** Verse renders worlds in the
> browser with WebGPU; nothing yet renders camera observations for a policy
> in batch. Nothing here runs yet.

A vision policy learns from images, so the simulator must produce images:
many cameras, many environments, every step, at resolutions small enough to
train on and fast enough not to dominate the step. Training does not need a
beautiful picture; it needs a picture whose statistics resemble a real
camera's, with depth and segmentation alongside for free. Presentation
renderers and observation renderers are therefore different things that
read the same state.

## What this chapter teaches

- Camera models: pinhole intrinsics, extrinsics, distortion.
- Batched rasterization for RGB, depth, segmentation and normals.
- Ray tracing for depth-of-field, shadows and reflections when realism is
  the point.
- Image-level domain randomization: lighting, textures, camera pose and
  sensor noise.
- Keeping rendering out of the physics: a read-only stage.

## The model

A pinhole camera with intrinsics `K` and pose `[R | t]` maps a world point
`X` to pixel `u`:

```jai
u ~ K (R X + t),   K = [[f_x, 0, c_x], [0, f_y, c_y], [0, 0, 1]]
depth(u) = (R X + t)_z of the nearest surface along the pixel's ray
```

A batched rasterizer renders `B × C` small images (environments times
cameras) in one pass by tiling them into a large target or by instancing
views, as Madrona's batch renderer does
([Shacklett et al., *An Extensible, Data-Oriented Architecture for
High-Performance, Many-World Simulation*, SIGGRAPH 2023](https://doi.org/10.1145/3592427)).

## The design in Psionic

Two renderer profiles. The *observation* profile is a batched rasterizer in
Coax-generated shaders (WGSL for WebGPU, MSL for Metal, and CUDA compute),
producing RGB, depth, segmentation and normal tensors straight into device
memory a policy reads without a copy (Coax R23). The *presentation* profile
is Verse's renderer, which reads the same state for people to watch.
Neither may write state. Image randomization specs are data with
provenance like physics randomization (chapter 179). Ray-traced realism is a
later profile, measured for whether it changes policy transfer before it is
built.

## What it builds

- `psionic/lib/sim/camera.coax` and observation shaders; tensor outputs
  with stated layouts.
- A fixture rendering 4,096 environments × 2 cameras at 128 × 128.

## Acceptance checks and oracles

- Projection of known points against `K[R|t]` exactly; depth against
  chapter 181's ray-cast depth on the same scene, within a pixel's footprint.
- Segmentation IDs match entity handles; every pixel's ID is a visible
  surface.
- Rendering never changes the state digest; batch invariance holds for
  images as for physics.
- Throughput reported per image and per environment, with zero recorded
  host copies.

## How Genesis does it

Genesis offers PyRender rasterization, LuisaRender ray tracing, the Madrona
batch renderer and Nyx, a Vulkan ray tracer that outputs RGB only
([rendering](https://genesis-world.readthedocs.io/en/latest/user_guide/getting_started/visualization.html),
[release notes](https://github.com/Genesis-Embodied-AI/genesis-world/releases)).
We plan one batched observation rasterizer we own, on our own targets
including the browser, and reuse Verse for presentation instead of adopting
external renderers.

## Dependencies

Chapters 165, 179, 181, 183 (meshes and textures). From Coax: R9 GPU
interfaces, R21 layouts, R23 zero-copy buffers.
