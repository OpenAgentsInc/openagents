# 11. The WGSL renderer

> **Status: unimplemented — design draft.** Today each realm draws through
> `coax/web/host.js` and its own hand-written WGSL, with Coax already
> translating, emitting and checking parts of it. This chapter plans one
> Aethyr renderer whose shaders are Coax procedures, emitted as WGSL for the
> browser and SPIR-V for native. Nothing here runs yet.

A realm's simulation ends each step with a state; drawing turns the
interpolated state (chapter 1) into pixels. Aethyr keeps that turn small and
checkable: the realm's Coax writes vertices, instances, uniforms and a draw
list into caller-owned buffers, and a fixed set of passes reads them. The
shaders are ordinary Coax, so the same procedure runs on the CPU in a
`#test`, as WGSL in the browser and as SPIR-V on Vulkan.

## What this chapter teaches

- The frame contract: what the realm writes, what the host uploads, and what
  the passes read.
- Passes as data: background, scene with depth, effects, HUD, each with its
  targets and bindings declared.
- One shader source lowered to two targets, and the CPU run of the same
  procedure as the oracle.
- Comparing a GPU frame with its reference within a stated tolerance.

## The method

A frame is a pure function of the drawn state and the viewport:

```jai
frame(state_α, viewport) -> { vertices[], instances[], uniforms[], draws[] }
draw   = { pass, first, count, instance, instances }
image  = passes(frame)          // bg, scene (depth), fx, hud, in that order
```

A pass declares its inputs (uniform block, storage buffers, texture layers,
samplers, a depth map from an earlier pass) and its targets. Bindings get
their group and number from one table, so the browser host and the native
host lay out the same resources. A shader is a Coax procedure over the
vector, matrix and texture structs of the shader library; on the CPU,
sampling is nearest or bilinear over texel centres, clamped to the edge.

The tolerance for a GPU against a reference is per value: on rgba16float
targets, `|a − b| ≤ 2e-3 · max(1, |a|)`, with a stated allowance for edge
pixels whose depth test lands within the two sides' rounding. Hashes used for
noise are integer permutations, exact in f32 on any hardware, never
`fract(sin(x) * 43758)`, whose last-bit error the scale amplifies.

## What exists today

- Browser path: realm Coax compiled to WebAssembly, drawn by
  [coax/web/host.js](../../coax/web/host.js) on WebGPU with 31 hand-written
  WGSL files in `apps/openagents-web/` (for example
  [everglade.wgsl](../../apps/openagents-web/everglade.wgsl)). The host's
  asset kinds (`data`, `storage`, `index`, `layers`, `pack`, `sound`) are
  listed at the top of host.js.
- The shader library: [verse/gpu/shader.coax](../../verse/gpu/shader.coax)
  (V2–V4, M4, textures and samplers with CPU bodies) and
  [verse/gpu/wgsl.coax](../../verse/gpu/wgsl.coax) (integer vectors, WGSL
  builtins and clamping conversions).
- Coax to SPIR-V: [verse/gpu/spirv.coax](../../verse/gpu/spirv.coax), then
  `coax spirv --vertex V --fragment F`; Vulkan hosts in `verse/gpu/` (`vks.c`
  pipelines, `vk-passes.c` pass specs, `vk-window.c`, `vk-display.c`).
- WGSL to Coax: [coax/web/wgsl-coax.coax](../../coax/web/wgsl-coax.coax)
  translates Everglade's WGSL to 161 + 14 checked procedures. Coax to WGSL:
  [wgsl-block.coax](../../coax/web/wgsl-block.coax) and
  [wgsl-module.coax](../../coax/web/wgsl-module.coax);
  [wgsl-compile.coax](../../coax/web/wgsl-compile.coax) compiles the result
  in headless Chrome's own WGSL compiler.
- Pass checks: [verse/gpu/pass-check.coax](../../verse/gpu/pass-check.coax)
  draws a pass spec in headless Chrome and on the host GPU and compares the
  targets; the phases and their results are in
  [docs/verse/native-gpu.md](../../docs/verse/native-gpu.md) (G0–G6).

What is missing is one renderer: every realm still owns its WGSL, its pass
order and its binding layout.

## The design in Aethyr

The renderer owns the pass graph and the binding table; a realm supplies
its shading procedures and fills the frame buffers. Shaders live in Coax
only; the WGSL a page loads is emitted at build time and checked, and no
hand-written WGSL remains (native-gpu G6). The draw list is the realm's
only way to submit work, so its counts are bounded by chapter 16's budgets.

## What it builds

- `aethyr/render/frame.coax`: the frame contract (buffer layouts, draw
  list, viewport) and its admission.
- `aethyr/render/passes.coax`: the pass graph and binding table, shared by
  the browser and native hosts.
- `aethyr/render/emit.coax`: build-time emission of each realm's WGSL module
  and SPIR-V from the same procedures.

## Acceptance checks and oracles

- Every shader procedure has a CPU `#test`; the emitted WGSL compiles in
  headless Chrome with no errors, and the SPIR-V passes `spirv-val` and the
  Vulkan validation layers with 0 errors and 0 warnings.
- A realm's pass on the GPU matches the same Coax run on the CPU within the
  per-value tolerance above, over a recorded frame's uniforms.
- The browser and native hosts read one binding table; a mismatch is a
  build failure.
- Each listed realm's golden screenshot (`coax/web/goldens/`) is unchanged
  after its move, except where a fix is visible and reviewed.

## Dependencies

Chapter 1 (the interpolated state drawing reads) and chapter 2 (camera and
screen spaces). Psionic's [compute subset](../../psionic/book/ch037-a-coax-compute-subset.md)
shares the lowering. From Coax: [chapter 17](../../coax/book/ch17-gpu.md),
R9 target-specific GPU interfaces and R10 the browser vertical slice.
