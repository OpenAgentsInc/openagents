# Rive Integration Plan For WGPUI

## Goal

Render `.riv` assets inside the existing WGPUI render pipeline and prove the path with a dedicated Autopilot desktop pane.

This plan replaces the earlier "just embed another runtime/renderer" framing. The shipping path should use our own WGPUI/WGPU renderer. It should not:

- spin up a second renderer or `wgpu::Surface`
- rasterize `.riv` into spritesheets or per-frame SVG
- treat `docs/plans/14270-30306-simple-fui-hud.riv` as the permanent runtime asset location

## Current Repo Reality

The integration has to fit the code that already exists:

- `crates/wgpui-core/src/scene.rs` is the draw contract the renderer consumes. Today it carries layered quads, text, curves, and meshes, plus `svg_quads` that sit outside the layer model.
- `crates/wgpui-render/src/renderer.rs` turns each scene layer into GPU buffers in `Renderer::prepare` and draws them in one pass in `Renderer::render_with_clear`.
- `crates/wgpui-render/src/svg.rs` is an SVG rasterizer for static icon/image usage. It is fine for toolbar glyphs. It is the wrong path for interactive `.riv` playback.
- `crates/wgpui/src/components/*` already contains reusable, product-agnostic visual components like `Heatmap`, `DotsGrid`, `RingGauge`, and `Scanlines`. A reusable Rive surface belongs in this layer, not in the app.
- `apps/autopilot-desktop/src/pane_renderer.rs` is the app-side pane dispatcher. Panes paint through `PaintContext`.
- A new desktop pane is never "just one file". The actual registration path runs through:
  - `apps/autopilot-desktop/src/app_state.rs`
  - `apps/autopilot-desktop/src/app_state_domains.rs`
  - `apps/autopilot-desktop/src/panes/mod.rs`
  - `apps/autopilot-desktop/src/panes/<pane>.rs`
  - `apps/autopilot-desktop/src/pane_renderer.rs`
  - `apps/autopilot-desktop/src/pane_registry.rs`
  - `apps/autopilot-desktop/src/pane_system.rs`
  - `apps/autopilot-desktop/src/input/tool_bridge.rs` if we want automation aliases

Two constraints from the current implementation matter immediately:

1. `Scene` is layered for quads, text, curves, and meshes, but not for SVG quads.
2. Desktop redraw is already driven by the main event loop in `apps/autopilot-desktop/src/input.rs`, with a 16 ms active cadence and a 50 ms idle cadence via `handle_about_to_wait`.

That means we do not need a second timing loop, but we do need a cleaner layered scene contract for vector/image drawables before Rive can fit properly.

## Ownership Split

This needs to follow the repo boundary rules in `docs/OWNERSHIP.md`.

| Layer | Owner | Responsibility |
| --- | --- | --- |
| `crates/wgpui-core` | product-agnostic | vector scene types and scene integration |
| `crates/wgpui-render` | product-agnostic | GPU compilation and rendering of vector batches |
| `crates/wgpui` | product-agnostic | Rive-facing surface/controller API |
| `apps/autopilot-desktop` | app-owned | preview pane, pane state, and app-specific input mapping |

The app should own "what do wallet/provider/runtime facts mean for this animation". The crates should own "how do we render a Rive artboard in WGPUI".

## Design Rules

1. Feed the existing `Scene -> Renderer::prepare -> Renderer::render_with_clear` path.
2. Keep the renderer inside `crates/wgpui-render`; no side renderer hidden behind the pane.
3. Keep the pane workbench app-owned and reusable surface logic crate-owned.
4. Start with a preview/workbench pane, not a hardwired production HUD replacement.
5. Treat the current `.riv` file in `docs/plans/` as a planning asset. Copy a runtime asset into `apps/autopilot-desktop/resources/rive/` when implementation starts.

## Target Architecture

### 1. Generic Vector Scene Support In WGPUI

The right foundation is not a Rive-only render pass. It is a generic, layered vector batch that WGPUI can render and that a Rive adapter can emit.

Add a product-agnostic vector module to `crates/wgpui-core`:

- `crates/wgpui-core/src/vector.rs`
- export it from `crates/wgpui-core/src/lib.rs`

Suggested types:

- `VectorBatch`
- `VectorPrimitive`
- `VectorPath`
- `VectorPaint`
- `VectorStroke`
- `VectorGradient`
- `VectorImageRef`
- `VectorTransform`
- `VectorClip`

Extend `crates/wgpui-core/src/scene.rs`:

- add `vector_batches: Vec<(u32, VectorBatch, Option<Bounds>)>`
- add `draw_vector_batch`
- include vector batches in `Scene::layers()`
- keep clip behavior consistent with quads/text/meshes
- fix image-like drawables to be layer-aware

For the image path, do one of these before landing Rive:

- promote `SvgQuad` to the layered model by storing layer and clip with it, or
- replace it with a general layered image primitive that SVG rasterization feeds into

Why this comes first:

- Rive needs layered fills, strokes, clips, transforms, gradients, and images.
- The current unlayered `svg_quads` path is already a sign that the scene contract needs to be cleaned up before a stateful vector runtime lands.

### 2. Native WGPUI Vector Renderer

Add the GPU-side vector compiler in `crates/wgpui-render`:

- `crates/wgpui-render/src/vector.rs`
- re-export it from `crates/wgpui-render/src/lib.rs`

Extend `crates/wgpui-render/src/renderer.rs`:

- add `PreparedVectorBatch` inside the layer-preparation path
- compile vector batches during `Renderer::prepare`
- render vector batches in layer order inside `Renderer::render_with_clear`
- keep one top-level render pass owned by `Renderer`

Reuse existing infrastructure where it is good enough:

- filled paths can compile down to triangle buffers similar to `MeshPrimitive`
- strokes can compile to line/strip buffers similar to the current curve/line path

Initial renderer feature bar:

- solid fills
- strokes
- transforms
- clip stack
- opacity
- linear gradients
- radial gradients
- embedded or referenced images
- deterministic batching and metrics

Do not route Rive through `crates/wgpui-render/src/svg.rs`. Keep `svg.rs` for static icon ingestion only.

### 3. Rive Runtime Surface In `crates/wgpui`

Add a neutral Rive module to the facade crate:

- `crates/wgpui/src/rive.rs`
- optionally `crates/wgpui/src/components/rive_surface.rs` if the final API reads better as a component

Do not bury this under `components::hud`. Rive is not HUD-specific.

Suggested public surface:

```rust
pub struct RiveSurface { /* persistent artboard/runtime state */ }
pub struct RiveController { /* input + playback control */ }

pub enum RiveInputValue {
    Bool(bool),
    Number(f32),
    Trigger,
}
```

Responsibilities:

- load `.riv` bytes
- hold artboard/state-machine instance state
- advance by frame delta
- accept named bool/number/trigger inputs
- emit a `VectorBatch` into `PaintContext.scene`

Important boundary:

- if we temporarily need a third-party loader/runtime to understand the `.riv` binary, hide it behind a thin adapter trait
- do not let `rive-rs`, Vello, or another renderer become the presentation path
- the long-term contract is `Rive -> VectorBatch -> WGPUI Renderer`

This is the only shape that keeps rendering ownership aligned with the repo.

### 4. Frame Cadence And Redraw

The desktop app already has an adequate redraw loop in `apps/autopilot-desktop/src/input.rs`.

Implementation rule:

- while any visible `RiveSurface` is playing, transitioning, or waiting on asset decode, it must report "needs redraw"
- `handle_about_to_wait` should include that state in its redraw decision, the same way it already does for provider animation and pending chat output

That means:

- no second loop
- no direct timer thread talking to the GPU
- no per-pane surface ownership

The Rive surface should integrate with the existing 16 ms active / 50 ms idle cadence, and request redraw while the artboard is unsettled.

### 5. Asset Packaging

The current file:

- `docs/plans/14270-30306-simple-fui-hud.riv`

is good as a planning artifact, but it is the wrong place to load runtime assets from.

When implementation starts:

- copy or rename the asset to `apps/autopilot-desktop/resources/rive/simple-fui-hud.riv`
- load it from there for runtime use

For the first working cut, the cleanest path is:

- store the file under `resources/rive/`
- `include_bytes!` it from the desktop app or from a crate-level example harness

That avoids early bundle-path problems and keeps the first prototype deterministic.

If we later want live-reload or user-selected assets, add that as a dev-only or explicit secondary path.

## Desktop Proof Pane

Start with a singleton workbench pane instead of wiring Rive straight into Provider Control or Earnings.

The closest existing patterns are:

- `PsionicViz` for a visual-only pane
- `LocalInference` and `AppleFmWorkbench` for panes that mix controls and a visualization surface

Add in `apps/autopilot-desktop`:

- `PaneKind::RivePreview` in `src/app_state.rs`
- `RivePaneState` in `src/app_state_domains.rs`
- `src/panes/rive.rs`
- `src/panes/mod.rs`
- `src/pane_renderer.rs`
- `src/pane_registry.rs`
- `src/pane_system.rs`
- `src/input/tool_bridge.rs` aliases if automation should open it

Recommended pane metadata:

- title: `Rive Preview`
- command id: `pane.rive_preview`
- aliases: `rive`, `rive_preview`, `hud_preview`
- singleton: `true`

Recommended pane state:

```rust
pub struct RivePaneState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub asset_name: String,
    pub artboard_name: Option<String>,
    pub state_machine_name: Option<String>,
    pub autoplay: bool,
    pub playing: bool,
    pub fit_mode: RiveFitMode,
    pub frame_build_ms: Option<f32>,
    pub draw_call_count: u32,
    pub last_pointer: Option<Point>,
}
```

Recommended runtime lifetime split:

- `RivePaneState` stores user-visible UI state
- the loaded `RiveSurface`/controller lives in `RenderState` as non-serialized runtime state
- reload swaps the runtime object but preserves selected artboard/state machine if still valid

Recommended first controls:

- reload asset
- play/pause
- restart animation
- cycle fit mode
- show current artboard/state machine
- show renderer metrics and missing-feature warnings

Input handling:

- if the first pane is mostly preview-only, add button hit targets in `pane_system.rs`
- if we later need freeform asset-path or input editors, then add a small `RivePaneInputs` struct in `app_state.rs`
- forward hover/down/up events to the Rive surface so interactive state machines can be exercised

## Concrete Work Plan

### Phase 0: Fix The Scene Contract

Files:

- `crates/wgpui-core/src/scene.rs`
- `crates/wgpui-core/src/lib.rs`
- `crates/wgpui-core/src/vector.rs`
- `crates/wgpui-render/src/renderer.rs`

Deliverables:

- layer-aware vector batches
- image-like drawables no longer bypass layering
- render metrics updated to count vector batches

Exit criteria:

- a static vector batch can be emitted in one pane layer and appear between pane background quads and pane text exactly where expected

### Phase 1: Static `.riv` Frame

Files:

- `crates/wgpui/src/rive.rs`
- `crates/wgpui-render/src/vector.rs`
- `apps/autopilot-desktop/src/panes/rive.rs`

Deliverables:

- load a packaged `.riv`
- select the default artboard
- emit a first-frame `VectorBatch`
- paint it inside a desktop pane without a second renderer

Exit criteria:

- the HUD asset renders at the correct bounds
- it respects pane clipping and resize
- it draws through the normal `Renderer::prepare/render_with_clear` path

### Phase 2: Time And State Machines

Files:

- `crates/wgpui/src/rive.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/panes/rive.rs`
- `apps/autopilot-desktop/src/app_state_domains.rs`

Deliverables:

- frame-delta advancement
- play/pause/restart controls
- named bool/number/trigger inputs
- hover/click forwarding
- redraw participation in `handle_about_to_wait`

Exit criteria:

- the pane can tick a state machine deterministically on the existing redraw cadence
- interaction works without adding a second event loop

### Phase 3: Productionize The Pane

Files:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`

Deliverables:

- full pane registration
- command-palette visibility
- automation aliases
- user-facing fallback states for load or feature failures

Exit criteria:

- `pane.rive_preview` opens from the command system
- repeated open/close/reload cycles are stable
- the pane survives resize and focus changes cleanly

### Phase 4: Embed Into Real Product Surfaces

After the preview pane is stable, reuse the same `RiveSurface` inside real product panes instead of keeping Rive isolated forever.

Likely first consumers:

- provider / earnings score surfaces
- onboarding / first-run celebratory HUD states
- wallet milestone feedback

Rule:

- embed the reusable `RiveSurface`
- do not fork a second app-local render path

## Data Flow

```text
apps/autopilot-desktop::RivePaneState
    -> wgpui::rive::RiveSurface
    -> wgpui_core::scene::draw_vector_batch(...)
    -> wgpui_render::Renderer::prepare(...)
    -> wgpui_render::Renderer::render_with_clear(...)
```

## Pane Wiring Checklist

The pane work is not done until all of these are wired:

- `apps/autopilot-desktop/src/app_state.rs`
  - add `PaneKind::RivePreview`
  - add `RenderState` storage for pane state and runtime object
- `apps/autopilot-desktop/src/app_state_domains.rs`
  - add `RivePaneState`
- `apps/autopilot-desktop/src/panes/mod.rs`
  - export the pane module
- `apps/autopilot-desktop/src/panes/rive.rs`
  - host the `RiveSurface`
  - paint controls and metrics
- `apps/autopilot-desktop/src/pane_renderer.rs`
  - dispatch `PaneKind::RivePreview`
- `apps/autopilot-desktop/src/pane_registry.rs`
  - register title, size, command id, singleton flag
- `apps/autopilot-desktop/src/pane_system.rs`
  - minimum size
  - hit boxes for controls
  - cursor behavior if the surface becomes interactive
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
  - pane aliases for automation
- `apps/autopilot-desktop/resources/rive/`
  - packaged runtime asset

## Validation

Code-level validation:

- `cargo test -p wgpui-core`
- `cargo test -p wgpui-render`
- `cargo test -p autopilot-desktop pane_registry`

Add focused tests for:

- vector batch layering
- clip-stack behavior
- renderer prepare determinism
- Rive surface redraw/settled behavior
- pane alias resolution

Manual desktop smoke:

1. open `pane.rive_preview`
2. verify the packaged asset loads
3. resize the pane and verify fit/alignment behavior
4. toggle play/pause/restart
5. click/hover the surface if the asset uses interactive inputs
6. confirm renderer metrics stay sane and no second surface/loop is created

## Explicit Non-Goals

Do not:

- render Rive through `svg.rs`
- export the asset to spritesheets or video for MVP
- attach a second `wgpu::Surface` or renderer loop to the pane
- load runtime assets from `docs/plans/`
- push app-specific provider/wallet logic down into `crates/wgpui*`

## Recommended First Implementation Order

1. Make scene drawables layer-aware enough for vector and image batches.
2. Land a minimal vector batch renderer in `wgpui-render`.
3. Land a reusable `RiveSurface` that emits that batch.
4. Add the singleton `Rive Preview` pane around the packaged HUD asset.
5. Only then start embedding the surface into other panes.
