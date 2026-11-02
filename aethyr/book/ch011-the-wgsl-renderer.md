# 11. The WGSL renderer

> **Status: unimplemented — design draft.** The shader port already emits
> the realms' WGSL and SPIR-V from Coax, and the native Vulkan path has
> recorded parity evidence. This chapter plans the shared Aethyr frame,
> pass and binding admission above those paths, together with a complete
> CPU reference for the admitted shader profiles. Those Aethyr modules
> are not built yet; the existing renderer is the baseline to preserve.

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
draw   = { first, count, base_instance, instances }  // four u32 words
image  = passes(frame)          // pass specs select their draws and resources
```

A pass declares its inputs (uniform block, storage buffers, texture layers,
samplers, a depth map from an earlier pass) and its targets. Bindings get
their group and number from one table, so the browser host and the native
host lay out the same resources. A shader is a Coax procedure over the
vector, matrix and texture structs of the shader library. The existing draw
record ABI is four `u32` words (`first`, `count`, base instance, instances),
as both hosts consume today; introducing admitted plans must preserve it.

The tolerance for a GPU against a reference is per value: on rgba16float
targets, `|a − b| ≤ 2e-3 · max(1, |a|)`, with a stated allowance for edge
pixels whose depth test lands within the two sides' rounding. New strict
profiles use integer noise hashes. Existing shader profiles that still use
`fract(sin(x) * 43758)` retain their recorded images until an explicit,
qualified migration: changing the hash can change a realm's appearance.

## What exists today

- Browser path: realm Coax compiled to WebAssembly, drawn by
  [coax/web/host.js](../../coax/web/host.js) on WebGPU. The native-GPU
  [status record](../../docs/verse/native-gpu.md#status) records all 31
  realm WGSL files as `coax wgsl` output, including
  [everglade.wgsl](../../apps/openagents-web/everglade.wgsl). The host's
  asset kinds (`data`, `storage`, `index`, `layers`, `pack`, `sound`) are
  listed at the top of host.js.
- The shader library: [verse/gpu/shader.coax](../../verse/gpu/shader.coax)
  (V2–V4, M4, textures and samplers with CPU bodies) and
  [verse/gpu/wgsl.coax](../../verse/gpu/wgsl.coax) (integer vectors, WGSL
  builtins and clamping conversions).
- Shader emission: `coax wgsl` and `coax spirv --vertex V --fragment F`
  lower the same Coax shader procedures through
  [coax/lib/compiler/wgsl.coax](../../coax/lib/compiler/wgsl.coax) and
  [coax/lib/compiler/shader.coax](../../coax/lib/compiler/shader.coax).
  [verse/gpu/spirv.coax](../../verse/gpu/spirv.coax) supplies the SPIR-V
  writer. The historical WGSL-to-Coax translator remains tooling, not the
  source that must be translated on each build.
- Native execution: Vulkan hosts in `verse/gpu/` (`vks.c` pipelines,
  `vk-passes.c` pass specs, `vk-window.c`, `vk-display.c`) and
  [verse/native/realm-host.c](../../verse/native/realm-host.c). The status
  record covers all 16 catalogue realms on the named RTX 4080 profile,
  native/Wasm game-frame comparisons and Vulkan pass validation. It does
  not qualify every GPU, the unfinished common Aethyr renderer or a full
  CPU raster reference.
- Pass checks: [verse/gpu/pass-check.coax](../../verse/gpu/pass-check.coax)
  draws a pass spec in headless Chrome and on the host GPU and compares the
  targets; the phases and their results are in
  [docs/verse/native-gpu.md](../../docs/verse/native-gpu.md) (G0–G6).

What is missing is one admitted renderer contract: realm-specific Coax
shaders and host paths still need shared frame/pass/binding admission.
The shader library's CPU texture operations cover simple nearest/bilinear
sampling, and its fragment derivatives currently return zero for an isolated
fragment. That is not an oracle for derivative-dependent shaders. A full
reference must model fragment quads and helpers, discard, texture levels and
filtering, raster/depth/blend rules and target rounding. Each admitted profile
must state which of these operations it supports.

## The design in Aethyr

The renderer owns the pass graph and the binding table; a realm supplies
its shading procedures and fills the frame buffers. It reuses the existing
Coax shader emission and four-word draw records. The draw list is the realm's
only way to submit work, so its counts are bounded by chapter 16's budgets.
Browser/native parity and CPU-reference qualification are separate evidence:
a derivative-free reference can qualify a limited profile, but cannot finish
this chapter's full renderer goal.

## What it builds

- `aethyr/render/frame.coax`: the frame contract (buffer layouts, draw
  list, viewport) and its admission.
- `aethyr/render/passes.coax`: the pass graph and binding table, shared by
  the browser and native hosts.
- `aethyr/render/emit.coax`: build-time emission of each realm's WGSL module
  and SPIR-V from the same procedures.

## Acceptance checks and oracles

- Every admitted shader profile has CPU checks, including nonzero quad
  derivative fixtures where derivatives are supported; the emitted WGSL compiles in
  headless Chrome with no errors, and the SPIR-V passes `spirv-val` and the
  Vulkan validation layers with 0 errors and 0 warnings.
- A realm's pass on the GPU matches the same Coax run on the CPU within the
  per-value tolerance above, over a recorded frame's uniforms.
- The browser and native hosts read one binding table; a mismatch is a
  build failure.
- Malformed frame/pass/binding plans are refused before either host submits
  partial GPU work. Both hosts retain the four-`u32` draw ABI.
- Each listed realm's golden screenshot (`coax/web/goldens/`) is unchanged
  after its move, except where a fix is visible and reviewed.

## Dependencies

Chapter 1 (the interpolated state drawing reads) and chapter 2 (camera and
screen spaces). Psionic's [compute subset](../../psionic/book/ch037-a-coax-compute-subset.md)
shares the lowering. From Coax: [chapter 17](../../coax/book/ch17-gpu.md),
R9 target-specific GPU interfaces and R10 the browser vertical slice.
