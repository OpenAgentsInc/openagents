> Deprecated (owner, 2026-10-24): history only. Architecture decisions live in [`docs/adr/`](../../../adr/README.md).

# 0007: GPU-driven rendering on every platform

Status: selected, 2026-10-24. Not implemented. Extends the
[Town renderer redesign](../town-renderer-redesign.md); its compiled batches
and material families are the first step toward this design.

## Context

Measured frames are limited by GPU work and by how the CPU submits it, not by
CPU language ([decision 0006](0006-cross-platform-engine-core.md)):

- **Town.** The main HDR pass takes 22–42 ms at 1280 × 720 with 4× MSAA on the
  recorded Mac. A frame with 100,000 creatures encodes about 8,100 source draws,
  1,670 pipeline binds, and 4,600 material binds
  ([compiled batches](../research/town-compiled-batches-2026-10-24.md)).
  Compiled batches removed 29% of draws but almost no binds, and gave no
  reliable frame-rate gain.
- **Battles.** The million-fighter wide view submits 146.85 million triangles
  per frame, with a 59.44 ms median full GPU time
  ([battle crowd](../battle-crowd.md)). Simulation is not the limit; full-detail
  meshes at every distance are.

Fewer CPU commands alone do not make frames faster. The GPU must do less work
and decide what to draw itself.

## Decision

1. **GPU-resident scene.** Instance transforms, bounds, semantic IDs, material
   parameters, and cluster tables live in storage buffers. Edits and simulation
   write deltas. A new generation swaps whole tables at activation, preserving
   [decision 0002](0002-incremental-world.md)'s complete-generation rule.
2. **Compute visibility.** Compute passes run frustum culling and two-phase
   hierarchical-depth occlusion culling, then write indirect draw arguments.
   The CPU records a fixed small set of passes regardless of scene size.
3. **Visibility buffer for opaque geometry.** Opaque passes write depth plus
   instance and triangle IDs; material families shade them afterward from
   material tables and texture arrays or virtual-texture pages. Picking and
   selection read the same IDs, so semantic identity survives. Clustered
   forward shading stays for transparency, effects, and as the fallback path.
4. **Temporal anti-aliasing and upscaling** replace 4× MSAA as the default.
   The renderer produces motion vectors and history from the start. UI,
   nameplates, and text render at output resolution.
5. **Cluster LOD for dense static geometry.** The offline compiler builds
   cluster hierarchies with meshoptimizer. Compute rasterization handles
   small triangles within WebGPU's 32-bit atomic limit.
6. **Crowd tiers.** Nearby characters use full skinning; mid-range characters
   use animation baked into textures; distant ones use impostors. A
   significance budget sets update rates. Simulation and combat queries keep
   full fidelity whatever the visual tier.
7. **Capability-selected fast paths.** Subgroups, timestamp queries,
   multi-draw-indirect, and bindless textures are optional features selected
   at startup, each with a baseline-WebGPU fallback that renders the same image.

The order of work follows expected payoff and cost: anti-aliasing first, then
compute visibility with indirect draws, then the visibility buffer, then
cluster LOD and crowd tiers.

## Alternatives and consequences

- **Keep CPU batching and tune passes one by one:** the current plan's first
  phase, kept as the bridge. Measurements show it does not remove the binds
  or triangle volume that dominate.
- **A Nanite-style renderer first:** high cost, and WebGPU lacks the 64-bit
  atomics and mesh shaders native versions use. Cluster LOD arrives after
  visibility and anti-aliasing are proven.
- **Hardware ray tracing:** absent from portable WebGPU. Lighting stays on
  baked probes and clustered lights, with compute techniques where measured.

Consequences: temporal reconstruction needs careful handling of thin foliage,
particles, teleports, and disocclusion. The GPU tables add memory that profile
budgets must account for. Tests compare moving-camera sequences, not only
still images.

## Revisit

Each step is an experiment against the current path on the pinned Town and
battle profiles at a fixed image target, with paired order and retained image
comparison ([performance qualification](../performance-qualification.md)).
Keep a step only if it reduces GPU time or raises accepted submissions without
failing readability or image checks. If temporal anti-aliasing fails text or
silhouette readability on a target, that profile keeps MSAA.
