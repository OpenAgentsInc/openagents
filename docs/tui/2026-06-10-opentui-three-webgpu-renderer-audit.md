# OpenTUI's Three.js / WebGPU Renderer (`@opentui/three`) — Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-10
Author: agent audit (Claude Code)
Reference repos (read-only lanes):
- `projects/repos/opentui` — `packages/three` (the subject), `packages/core`, `packages/examples`
- `projects/repos/opencode` — usage check (clone at 2026-06-05)

Companion docs in this folder: the Textual and opencode TUI audits, the
parity roadmap (#4736–#4742, all shipped), and the opencode Claude
integration audit.

## TL;DR

`@opentui/three` (v0.3.4, published in lockstep with the rest of OpenTUI) is
a real GPU-accelerated 3D renderer for terminals: Three.js scenes are
rendered through Three's WebGPU backend against a **native WebGPU device
inside Bun** (no browser), read back as pixels, and quantized by a GPU
compute shader into terminal cells — each cell becoming one of 16 Unicode
quadrant-block glyphs plus a two-color foreground/background pair. On top of
the renderer sits a small game-engine layer: instanced sprite-sheet
animation, procedural textures, particle/explosion effects, and optional 2D
physics adapters (Rapier, Planck).

**opencode does not use it.** Its TUI's celebrated animated logo is a
CPU-procedural effect written directly against OpenTUI's `OptimizedBuffer`,
and the only WebGPU in the opencode monorepo is a browser-side visual in
their web console app, unrelated to OpenTUI. Details in §8.

## 1. Package identity

- **Name/version:** `@opentui/three` 0.3.4 — versioned and released with
  `@opentui/core` (latest release 2026-06-07; active cadence).
- **Dependencies:** `three@0.177.0` (exact), `bun-webgpu@0.1.7`,
  `jimp@1.6.1` (image decode for textures), optional
  `@dimforge/rapier2d-simd-compat@^0.17.3` and `planck@^1.4.2` (physics).
- **Engines:** Bun ≥ 1.3.0. Ships TypeScript source directly
  (`main: src/index.ts`), like the rest of the monorepo. No package README;
  no tests in the package (examples serve as integration coverage). A
  2026-05-11 commit adds "Node.js 26 runtime support" to the *examples*,
  suggesting Node support is being explored, but the engines field remains
  Bun.
- **Scale:** ~1,525 LOC of source across 16 files, plus a 202-line WGSL
  compute shader and a benchmark suite.

Key files (paths under `packages/three/src/`):

| File | ~LOC | Role |
|---|---|---|
| `WGPURenderer.ts` | 292 | `ThreeCliRenderer`: device/canvas creation, sizing, camera aspect, stats |
| `canvas.ts` | 462 | `CLICanvas`: the fake HTMLCanvasElement, texture readback, supersampling dispatch |
| `shaders/supersampling.wgsl` | 202 | GPU compute: 2×2 pixels → quadrant glyph + 2 colors |
| `ThreeRenderable.ts` | 200 | OpenTUI `Renderable` wrapper: frame loop, resize, lifecycle |
| `SpriteAnimator.ts` | 633 | Instanced sprite-sheet animation state machine (TSL materials) |
| `SpriteResourceManager.ts` | 286 | Texture loading/caching, `InstancedMesh` slot pooling |
| `ExplodingSpriteEffect.ts` / `SpriteParticleGenerator.ts` | 948 | Particle/explosion effects (self-contained physics) |
| `physics/{Rapier,Planck}PhysicsAdapter.ts` | 140 | Optional 2D rigid-body adapters |
| `TextureUtils.ts` | 196 | Procedural textures (checkerboard/gradient/noise) via `DataTexture` |

## 2. How a Three.js scene becomes terminal cells

The pipeline, end to end:

```
Three.js Scene + Camera
  → three/webgpu WebGPURenderer            (real GPU work)
  → render target texture (RGBA8/BGRA8), 2× the terminal cell grid
  → readback (zero-copy via Bun FFI mapped-range pointer)
  → supersampling pass (GPU compute / CPU-Zig / none)
  → per-cell {glyph, fg, bg} results
  → OptimizedBuffer (OpenTUI's Zig-backed cell buffer)
  → native Zig diff renderer → ANSI to the terminal
```

The load-bearing trick is the **cell quantization**: with 2× supersampling, a
terminal cell corresponds to a 2×2 pixel block. The compute shader
(`supersampling.wgsl`) does, per cell:

1. Find the two most mutually distant colors among the 4 pixels (Euclidean
   RGB).
2. Assign them dark/light by luminance (`0.2126R + 0.7152G + 0.0722B`).
3. Classify each of the 4 pixels to the nearer color, producing a 4-bit
   quadrant mask.
4. Look up the mask in a 16-entry table of Unicode quadrant blocks
   (` ▗▖▄▝▐▞…█`), output `{char, fg, bg}` as a packed 48-byte `CellResult`
   struct (with alpha blending for transparency).

The packed results buffer is read back and handed to
`OptimizedBuffer.drawPackedBuffer()` — native Zig unpacks straight into the
cell arrays. So one terminal cell carries 2 colors + 1 of 16 glyphs, i.e.
effectively a 2×2 monochrome-per-color sub-pixel display. There is no
dithering/error diffusion; the quadrant choice is the quantizer.

Three supersampling modes (`SuperSampleType`): `GPU` (the compute path,
default), `CPU` (raw pixels handed to a Zig-side implementation via
`drawSuperSampleBuffer`), and `NONE` (each pixel becomes a full-block `█`
cell — fastest, crudest). The GPU path also has two algorithms
(`STANDARD` 4-distinct-pixels vs `PRE_SQUEEZED` horizontal pre-blend,
selectable at runtime).

A detail worth stealing even without 3D: the camera aspect ratio is computed
as `width / (height * 2)` because terminal cells are ~2× taller than wide —
the same cell-geometry correction any TUI graphics work needs.

## 3. WebGPU without a browser

- The GPU device comes from **`bun-webgpu`** (v0.1.7, by `kommander` — an
  OpenTUI contributor; "Native WebGPU implementation for Bun runtime"):
  native bindings exposed to Bun via FFI, with `setupGlobals({ libPath })`
  allowing an explicit native library override (Dawn/wgpu, Metal on macOS,
  Vulkan elsewhere).
- The `HTMLCanvasElement` Three expects is faked by `CLICanvas`:
  `getContext("webgpu")` returns bun-webgpu's `GPUCanvasContextMock` with
  double-buffered textures (`getCurrentTexture()` / `switchTextures()`),
  256-byte row alignment per the WebGPU spec, and format negotiation
  (BGRA8 on macOS, RGBA8 elsewhere).
- Readback uses a Bun extension, `getMappedRangePtr()` — a raw FFI pointer
  into the mapped GPU buffer, so pixels move to the Zig cell buffer without
  a JS-side copy.
- **No fallback path exists**: no WebGL, no software renderer, no feature
  detection. If the native WebGPU device can't initialize, the package
  fails. This is the sharpest maturity caveat.

## 4. Integration with OpenTUI core

`ThreeRenderable` is an ordinary OpenTUI `Renderable` (buffered, with width/
height/position/flex props), so a 3D viewport composes into normal TUI
layout — including absolute positioning, z-index, and mouse events. It:

- asserts the context is a `CliRenderer` and registers an async **frame
  callback** (`setFrameCallback`) that renders the scene into its frame
  buffer each tick, honoring `targetFps` from the renderer;
- handles `onResize` by resizing the GPU canvas and updating camera aspect
  (`autoAspect: true` by default);
- supports alpha (`backgroundColor` with alpha + per-cell alpha blending),
  so 3D content can float transparently over TUI content;
- cleans up GPU resources and unregisters callbacks on `destroy()`.

The examples show a `DraggableThreeRenderable` subclass implementing
`onMouseEvent` for drag-the-viewport behavior — 3D panes participate in the
same mouse pipeline as any widget.

There are **no React or Solid bindings**: no `<three>` intrinsic, no
react-three-fiber-style adapter in `@opentui/react` or `@opentui/solid`.
Usage is imperative Three.js plus one renderable. (For our Solid-based Pylon
view, embedding one would mean a `ref`-mounted imperative island.)

## 5. Minimal usage (from `packages/examples/src/draggable-three-demo.ts`)

```ts
import { createCliRenderer, RGBA } from "@opentui/core"
import { ThreeRenderable } from "@opentui/three"
import { Scene, Mesh, BoxGeometry, MeshPhongMaterial, AmbientLight, Color } from "three"

const renderer = await createCliRenderer({ targetFps: 60 })
renderer.start()

const scene = new Scene()
scene.add(new AmbientLight(new Color(0.35, 0.35, 0.35), 1.0))
const cube = new Mesh(new BoxGeometry(1, 1, 1), new MeshPhongMaterial({ color: new Color(0.25, 0.8, 1.0) }))
scene.add(cube)

renderer.root.add(
  new ThreeRenderable(renderer, {
    id: "cube-view",
    width: 64, height: 40, position: "absolute", left: 10, top: 5,
    scene,
    renderer: { focalLength: 8, alpha: true, backgroundColor: RGBA.fromValues(0, 0, 0, 0) },
  }),
)

renderer.setFrameCallback(async (deltaMs) => {
  cube.rotation.x += 0.6 * (deltaMs / 1000)
  cube.rotation.y += 0.4 * (deltaMs / 1000)
})
```

The 1,014-line `shader-cube-demo.ts` shows the lower-level path: using
`ThreeCliRenderer` directly (no renderable wrapper) with custom TSL/node
materials, composed with OpenTUI's post-processing filters (noise, chromatic
aberration) applied to the *cell buffer* after the 3D pass — GPU 3D and
CPU cell-space post-FX stack cleanly.

## 6. The game-engine layer

Beyond scene rendering, the package ships a compact 2D-game toolkit:

- **`SpriteResourceManager`**: loads sprite sheets via Jimp into
  `DataTexture`s (NearestFilter, no mipmaps), manages pooled
  `InstancedMesh`es with per-instance frame-index and flip attributes.
- **`SpriteAnimator`**: frame-based animation state machine per sprite
  (named animations over sheet ranges, frame duration, looping,
  `setAnimation`/`goToFrame`), with TSL node materials sampling the sheet by
  per-instance UV offset — thousands of animated sprites in one draw call.
- **`ExplodingSpriteEffect` / `PhysicsExplodingSpriteEffect` /
  `SpriteParticleGenerator`**: split a sprite into an N×M fragment grid and
  animate fragments with velocity/gravity/rotation — self-contained, or
  driven by the **Rapier2D/Planck adapters** behind a small
  `PhysicsWorld`/`PhysicsRigidBody` interface.
- **`TextureUtils`**: procedural checkerboard/gradient/noise textures.

No GLTF/asset-pipeline support, no text-in-3D helpers, no texture
compression. This layer targets retro 2D/sprite games in the terminal more
than general 3D apps.

## 7. Performance machinery and maturity

- Built-in stats (toggleable overlay): GPU render ms, readback ms (split
  into mapAsync wait and supersample draw), total draw ms, active
  supersample mode/algorithm.
- A benchmark suite (`benchmark/renderer-benchmark.ts`) measuring FPS and
  memory across single-cube/multi-cube/textured/full-scene scenarios.
- Hardcoded `NoToneMapping` and `LinearSRGBColorSpace` (no gamma option),
  single render target, 16-glyph quantization ceiling.
- **No tests, no README, no fallback** — production-active (released in
  lockstep with core) but clearly the most experimental package in the
  OpenTUI family. Treat the public API as livelier than core's.

## 8. Does opencode use it? No — here's what they do instead

Searched the opencode monorepo (clone of 2026-06-05) for `@opentui/three`,
`three` imports, `bun-webgpu`, and WebGPU references:

1. **The opencode TUI uses none of it.** No `@opentui/three` or `three`
   dependency anywhere in the TUI. The famous animated home-screen logo —
   the thing one might assume is GPU work — is a **CPU-procedural effect**:
   `packages/opencode/src/cli/cmd/tui/component/bg-pulse-render.ts` (436
   LOC) plus `logo.tsx` (885 LOC) write ring/pulse/shimmer math directly
   into OpenTUI's `OptimizedBuffer` using half-block (`▀`) and full-block
   (`█`) glyphs, a precomputed logo template of typed cells, and
   per-frame RGBA writes. Same cell-buffer primitives `@opentui/three`
   ultimately targets — minus the GPU, the readback, and the dependency.
   For a bounded brand animation, CPU cell-painting was the right call, and
   it's the pattern to copy for any Pylon "brand moment."
2. **The only WebGPU in opencode is in the browser, unrelated to OpenTUI:**
   `packages/console/app/src/component/spotlight.tsx` (820 LOC, their web
   console app) hand-rolls a browser WebGPU device/context with WGSL
   shaders (`@webgpu/types` dev dep, no Three.js) for a spotlight/particles
   background, with graceful "WebGPU not supported" degradation. Different
   runtime, different renderer, zero overlap with `@opentui/three`.

So the canonical OpenTUI consumer treats the 3D package as what it is: a
showcase/games capability, not product infrastructure.

## 9. Relevance to Pylon

- **Nothing in the current roadmap needs it.** Our dashboard (#4736–#4742)
  is text/data-dense; none of its surfaces benefit from a 3D viewport, and
  adding `bun-webgpu` would attach a native GPU dependency (with no
  fallback) to a node whose job is unattended earning on heterogeneous
  hardware. opencode's restraint here is the right model.
- **If we ever want visual flair** (a startup brand moment, an earnings
  celebration), the opencode logo pattern — CPU cell-painting against
  `OptimizedBuffer` with half-blocks — gets there with zero new
  dependencies and stays compatible with our headless test harness
  (`src/tui/harness.tsx`), which captures character frames. `@opentui/three`
  output would also land in the same cell buffer and is in principle
  harness-capturable, but tests would then require a working native WebGPU
  device on CI — a hard sell.
- **Two transferable ideas regardless:** the quadrant-glyph quantization
  (2×2 sub-cell resolution with two colors per cell) is the standard
  technique if we ever render charts/sparklines with more resolution than
  one glyph per cell (Phase-future telemetry graphs); and the
  `width/(height*2)` cell-aspect correction applies to any terminal
  graphics math.
- **If a real use case appears** (e.g., a 3D visualization of cluster
  topology for operators), the integration shape is clear: an imperative
  `ThreeRenderable` island mounted via `ref` inside our Solid tree, gated
  behind an optional dependency and a capability probe, never on the
  default startup path.

## Bottom line

`@opentui/three` is a genuinely clever piece of engineering — a full
Three.js WebGPU pipeline running headless in Bun via native FFI, quantized
to quadrant glyphs by a compute shader, composed into ordinary TUI layout as
a draggable, alpha-blended renderable, with an instanced sprite/particle/
physics layer on top. It is also the least production-hardened package in
the OpenTUI family (no tests, no fallback, hardcoded color pipeline), and
the ecosystem's flagship consumer ships none of it: opencode's signature
animation is CPU cell-painting, and its only WebGPU lives in a browser app.
For Pylon the verdict is the same as opencode's — admire it, borrow the
cell-quantization math and the CPU brand-animation pattern when wanted, and
keep GPU dependencies out of the earning node.
