Yes — the closest thing to an “exact import” path is to export the animation as a **`.riv` runtime file** and render that inside your Rust app with Rive’s runtime, rather than trying to convert it into some generic animation format. Rive’s docs treat `.riv` as the runtime export, and their Rust runtime can load `.riv` files directly. ([GitHub][1])

The important catch is that **“exact” depends on the renderer**. Rive says different platforms can use different renderers with varying feature support, and their goal with the newer Rive Renderer is better fidelity to what you see in the editor. The current `rive-rs` repo says its existing Rust runtime uses **Vello** today, and that this backend has known visual differences versus the original design; they are working on making the official Rive Renderer available as another backend. ([rive.app][2])

So, in practice:

* **Best path for near-exact playback in WGPUI:** ship the `.riv` file and integrate `rive-rs` into your render layer. ([GitHub][1])
* **Not really an exact path:** exporting to spritesheets, JSON, SVG, or manually rebuilding the motion in your own Rust animation system. That can reproduce the look, but it is not the same as running the original interactive/state-machine asset. Rive’s runtime model is built around artboards, state machines, and assets inside the `.riv` file. ([Rive][3])

A few details that matter for your app architecture:

* If the animation uses nested artboards/components you want accessible at runtime, those artboards should be flagged as **components**, because only component-marked artboards are exported to the `.riv` file. ([Rive][3])
* If the file uses fonts, images, or audio, Rive supports **embedded**, **hosted**, or **referenced** asset export modes, which affects whether your Rust app must load assets itself. ([Rive][4])

My honest bottom line: **yes, you can export Rive in a format suitable for your Rust/WGPUI desktop app, and that format is `.riv`; but “exact” only really means exact if you embed Rive playback, not if you translate it into another animation system.** With `rive-rs` today, expect possible rendering deltas until the official Rive Renderer backend is available in Rust. ([GitHub][1])

For your stack, I’d treat it like this:

1. Designers ship `.riv`.
2. WGPUI hosts a Rive surface/widget.
3. Rust code drives state-machine inputs.
4. Any unsupported fidelity edge cases get tested against the current `rive-rs` backend early.

If you want, I can sketch the exact integration plan for **WGPUI + `rive-rs`**, including how to wrap it as a reusable component.

[1]: https://github.com/rive-app/rive-rs "GitHub - rive-app/rive-rs · GitHub"
[2]: https://rive.app/renderer "Rive Renderer"
[3]: https://help.rive.app/editor/fundamentals/nested-artboards "Components (formerly Nested Artboards) - Rive"
[4]: https://help.rive.app/runtimes/loading-assets "Loading Assets - Rive"

---

Yes. The right target is **not** “convert Rive to our scene graph,” but **build a Rust renderer/runtime layer that can consume `.riv` assets and execute the same artboard + animation + state-machine model inside WGPUI**. That matches how Rive itself is structured: the `.riv` file contains artboards and their contents; runtimes query linear animations and state machines from artboards; playback advances the artboard over time; and the C++ runtime explicitly exposes an abstract renderer interface for plugging in an external vector renderer. ([GitHub][1])

One important reality check first: Rive has open-source runtimes and renderer code, but I did **not** find a clean official public “binary `.riv` file format spec” intended for third-party reimplementation. Their public docs frame Rive as an editor + stateful graphics format + runtime + renderer stack, and their current public interfaces emphasize loading `.riv` files through the runtime rather than documenting the binary format for others to reimplement from scratch. ([Rive][2])

So the best spec is a **compatibility renderer spec**, not a speculative binary-format reverse-engineering spec. Concretely: parse `.riv` either by binding to or porting the open runtime model, then render with your own WGPU/WGPUI backend.

# OpenAgents / WGPUI Rive-Compatible Renderer Spec

**Status:** Draft v0.1
**Goal:** Render `.riv` content inside the Rust/WGPUI desktop app with high fidelity, deterministic timing, and tight integration with the app event loop.

## 1. Scope

This spec defines a Rust-native subsystem, `oa_rive`, that:

* loads exported `.riv` assets,
* instantiates artboards,
* advances animations and state machines over time,
* resolves data-bound inputs,
* produces GPU draw commands for WGPU,
* composites the result inside WGPUI surfaces,
* supports embedded and referenced assets,
* preserves enough of Rive semantics that designers can hand off `.riv` files without manual recreation.

This subsystem is explicitly modeled around public Rive runtime concepts:

* `.riv` files contain artboards and their contents,
* artboards expose linear animations and state machines,
* state machines are advanced frame-by-frame over time,
* runtime control occurs indirectly through inputs/data bindings rather than deep mutation of internal state,
* assets may be embedded, hosted, or referenced. ([GitHub][1])

## 2. Non-goals

Version 1 does **not** attempt to:

* define a brand-new authoring format,
* provide a full Rive editor replacement,
* guarantee bit-identical raster output versus Rive’s own renderer,
* support every future Rive feature on day one,
* support hosted asset loading from Rive CDN by default.

Rive itself notes that different platforms use different renderers with varying feature support, and that visual consistency has historically varied by renderer. ([Rive][3])

## 3. Product intent

For OpenAgents/WGPUI, this should be treated as a **UI animation runtime**, not merely a media player.

Primary use cases:

* onboarding flows,
* animated HUD widgets,
* wallet / earnings feedback,
* expressive stateful controls,
* mascot/orb/avatar systems,
* reactive micro-interactions tied to app state.

## 4. Architecture overview

The system is divided into six layers:

1. **Container Loader**
   Reads `.riv` bytes and produces an internal document model.

2. **Document Model**
   Immutable file-level objects: artboards, assets, animations, state machines, shapes, paints, text runs.

3. **Instance Layer**
   Mutable live instances of artboards and state machines, with input values and animation clocks.

4. **Layout + Fit Layer**
   Maps artboard coordinates into WGPUI layout rectangles.

5. **Renderer Backend**
   Converts resolved vector content into WGPU draw passes.

6. **Host Integration Layer**
   Connects to WGPUI event loop, input system, focus, hit-testing, and invalidation.

## 5. Compatibility strategy

There are two viable implementation paths.

### Path A: Hybrid compatibility

Use Rive’s open runtime as the semantic source of truth, but replace only the renderer backend.

This is the fastest path because the public C++ runtime already supports:

* loading `.riv`,
* querying artboards,
* querying animations and state machines,
* advancing artboards,
* external renderer integration. ([GitHub][1])

### Path B: Full Rust-native runtime

Port enough of the runtime semantics into Rust so that `.riv` files can be loaded and advanced entirely natively.

This yields deeper control and cleaner WGPUI integration, but costs more and carries format-drift risk if Rive changes the binary encoding or semantic model.

**Recommendation:** do Path A first, but define internal Rust traits so the host app does not care whether semantics come from FFI or a native port.

## 6. Core data model

### 6.1 File

A `RiveFile` is an immutable parsed asset.

```rust
pub struct RiveFile {
    pub version: FileVersion,
    pub artboards: Vec<ArtboardDef>,
    pub assets: Vec<AssetDef>,
    pub metadata: FileMetadata,
}
```

### 6.2 Artboard

An artboard is the root runtime composition unit.

```rust
pub struct ArtboardDef {
    pub name: String,
    pub width: f32,
    pub height: f32,
    pub nodes: Vec<NodeDef>,
    pub animations: Vec<LinearAnimationDef>,
    pub state_machines: Vec<StateMachineDef>,
    pub default_state_machine: Option<usize>,
}
```

This mirrors Rive’s runtime model where rendering selects an artboard and optionally a specific state machine; otherwise the default is used. ([Rive][4])

### 6.3 Live instance

```rust
pub struct ArtboardInstance {
    pub def: Arc<ArtboardDef>,
    pub world_state: WorldState,
    pub inputs: InputStore,
    pub active_linear_animations: Vec<LinearAnimationInstance>,
    pub active_state_machine: Option<StateMachineInstance>,
    pub needs_solve: bool,
}
```

### 6.4 Inputs

Expose only stable, public input handles to host code.

```rust
pub enum InputValue {
    Bool(bool),
    Number(f32),
    Trigger,
}
```

This follows Rive’s public runtime philosophy: state machines are meant to be controlled indirectly through inputs/data bindings rather than arbitrary direct mutation of internal state. ([Rive][4])

## 7. Timing and advancement model

The engine advances in host frame time.

```rust
fn advance(instance: &mut ArtboardInstance, dt_seconds: f32);
```

Advance order:

1. consume queued input changes,
2. advance active state machine,
3. evaluate active linear animations,
4. solve transforms, deformations, constraints, and property propagation,
5. mark paint/geometry caches dirty as needed,
6. emit renderable scene.

This reflects Rive’s runtime docs: state machines advance once per frame by delta time, evaluating keyframes, transitions, data-binding changes, and visible artboard elements. Runtimes may settle when nothing else changes. ([Rive][4])

### 7.1 Settling

Instances may enter `Settled` state when:

* no active transition is progressing,
* no keyframed value changes remain,
* no input changed this frame,
* no hover/pointer/focus event is pending.

When settled, the instance is skipped by the update loop until externally invalidated. This mirrors Rive’s documented settling optimization. ([Rive][4])

## 8. Rendering model

## 8.1 Render abstraction

```rust
pub trait VectorRenderer {
    fn begin_frame(&mut self, target: &RenderTarget, clear: Option<Color>);
    fn draw_path(&mut self, path: &ResolvedPath, paint: &ResolvedPaint, transform: Mat3);
    fn draw_image(&mut self, image: &ResolvedImage, transform: Mat3, opacity: f32);
    fn draw_text(&mut self, text: &ResolvedTextRun, transform: Mat3);
    fn push_clip(&mut self, clip: &ResolvedClip, transform: Mat3);
    fn pop_clip(&mut self);
    fn end_frame(&mut self);
}
```

The runtime layer must emit an ordered display list independent of backend.

## 8.2 Backend target

Initial backend: `wgpu`.

Backend design goals:

* one pass encoder per WGPUI frame subtree,
* path fill and stroke support,
* high-quality antialiasing,
* gradient support,
* clip stack,
* transform stack,
* opacity layers,
* text rendering via glyph atlas or analytic text path mode,
* predictable batching.

## 8.3 Coordinate systems

Artboards are 2D and should be treated as orthographic UI scenes. Fit and alignment policy must be explicit at embed time.

```rust
pub enum Fit {
    Fill,
    Contain,
    Cover,
    FitWidth,
    FitHeight,
    None,
    ScaleDown,
}

pub enum Alignment {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}
```

## 9. Scene emission pipeline

The solver produces a retained but frame-stamped display list.

```rust
pub struct DisplayList {
    pub commands: Vec<DrawCommand>,
    pub bounds: Rect,
    pub generation: u64,
}
```

`DrawCommand` variants:

* `PushTransform`
* `PopTransform`
* `PushClip`
* `PopClip`
* `DrawPathFill`
* `DrawPathStroke`
* `DrawImage`
* `DrawText`
* `PushOpacityLayer`
* `PopOpacityLayer`

This keeps runtime semantics separate from WGPU implementation details.

## 10. Asset loading

Rive documents three asset modes: embedded, hosted, and referenced. Embedded assets are in the `.riv`; hosted assets may be loaded from Rive’s CDN; referenced assets are loaded by the application through a handler. ([Rive][5])

For OpenAgents, define:

```rust
pub trait AssetResolver {
    fn resolve_image(&self, asset_id: AssetId, hint: &AssetHint) -> Result<ImageAsset>;
    fn resolve_font(&self, asset_id: AssetId, hint: &AssetHint) -> Result<FontAsset>;
    fn resolve_audio(&self, asset_id: AssetId, hint: &AssetHint) -> Result<AudioAsset>;
}
```

### Policy

* **Embedded:** supported in v1.
* **Referenced:** supported in v1.
* **Hosted via Rive CDN:** disabled by default in desktop builds; opt-in only.

Reason: desktop determinism, offline support, and fewer external dependencies.

## 11. WGPUI integration

Expose a widget/component:

```rust
pub struct RiveView {
    pub file: Arc<RiveFile>,
    pub artboard: Option<String>,
    pub state_machine: Option<String>,
    pub autoplay: bool,
    pub fit: Fit,
    pub alignment: Alignment,
}
```

Host-facing methods:

```rust
impl RiveViewHandle {
    pub fn play(&self);
    pub fn pause(&self);
    pub fn stop(&self);
    pub fn set_bool(&self, name: &str, value: bool);
    pub fn set_number(&self, name: &str, value: f32);
    pub fn fire_trigger(&self, name: &str);
}
```

This matches the public runtime concepts Rive exposes across platforms: autoplay, play/pause/stop, choosing state machine by name, and driving behavior through inputs. ([Rive][4])

### 11.1 Invalidation contract

A `RiveView` requests redraw when:

* advancing while not settled,
* an input changes,
* hover/press/focus changes,
* referenced assets finish loading,
* layout rect changes.

### 11.2 Threading

* Parse off main thread.
* GPU resource creation on render thread/device queue owner.
* Live instance mutation on UI thread unless a lock-free command queue is adopted.

## 12. Events and interaction

The runtime host may forward:

* pointer move,
* pointer enter/leave,
* pointer down/up,
* focus/blur,
* visibility changes.

Because Rive’s state machine runtime model is intentionally indirect, host interaction should be mapped to named inputs instead of poking internal states. ([Rive][4])

Recommended convention:

* `hovered: bool`
* `pressed: bool`
* `clicked: trigger`
* `focused: bool`
* `value: number`

## 13. Fidelity requirements

### 13.1 Required visual features for v1

* solid fills
* strokes
* linear gradients
* radial gradients
* transforms
* opacity
* clipping
* images
* basic text
* nested artboard rendering if present in parsed model

### 13.2 Stretch features

* blur
* drop shadow
* blend modes beyond normal
* mesh deformation optimization
* advanced text shaping parity
* audio sync hooks

Rive states that cross-platform renderer differences affect which features are consistently supported and that new features like blur and drop shadow have historically been limited by renderer differences. ([Rive][3])

## 14. Performance targets

For desktop OpenAgents UI:

* cold parse of small UI `.riv`: under 15 ms on a modern laptop
* warm instance creation: under 2 ms
* steady-state idle settled view: zero per-frame solver cost
* active lightweight animation: under 0.5 ms CPU/frame median
* GPU budget per typical HUD animation: under 0.75 ms/frame at 60 fps on integrated graphics

## 15. Binary compatibility policy

Because there is no public authoritative binary `.riv` spec intended for reimplementers, compatibility must be version-gated. ([Rive][2])

Define:

```rust
pub enum CompatibilityMode {
    StrictKnownVersions,
    BestEffortForwardCompatible,
}
```

### Rules

* Accept only tested file-version ranges by default.
* Log unsupported object types explicitly.
* Fail soft on unknown optional features.
* Fail hard on unknown core graph/object encodings.

## 16. Testing strategy

### 16.1 Golden semantic tests

For a curated asset corpus:

* selected artboard names parse correctly,
* selected state machine names resolve,
* input changes produce expected property values,
* settle behavior matches expectations.

### 16.2 Golden visual tests

Render a fixed asset set at deterministic timestamps:

* 0 ms
* 100 ms
* 250 ms
* 500 ms
* 1000 ms

Compare output against approved PNG references with tolerance thresholds.

### 16.3 Cross-runtime comparison

For each test asset:

* render using official runtime/renderer path where available,
* render using `oa_rive`,
* diff images and report drift.

## 17. Observability

Expose debug HUD:

* current artboard
* current state machine
* current inputs
* settle/playing/paused state
* frame advance dt
* display list command count
* path count
* clip depth
* GPU draw call count
* unsupported feature flags encountered

This fits your broader preference for robust telemetry and explicit state.

## 18. Implementation phases

### Phase 0: Feasibility

* bind to open C++ runtime
* load `.riv`
* choose artboard
* play one state machine
* render bounding boxes or simple paths in WGPU

### Phase 1: Hybrid renderer MVP

* C++ runtime semantics
* Rust WGPU renderer backend
* embedded assets
* play/pause/stop
* bool/number/trigger inputs
* WGPUI widget wrapper

### Phase 2: Production parity

* clips
* gradients
* text
* referenced assets
* settle/invalidation
* snapshot testing

### Phase 3: Native runtime migration

* incremental Rust port of semantic model
* FFI path retained as fallback
* asset corpus validates equivalence

## 19. Recommended crate layout

```text
crates/
  oa_rive/
    src/lib.rs
    src/file/
    src/model/
    src/instance/
    src/solver/
    src/display_list/
    src/assets/
    src/widget/
  oa_rive_wgpu/
    src/lib.rs
    src/pipelines/
    src/path_tessellation/
    src/text/
    src/images/
    src/clips/
  oa_rive_ffi/
    src/lib.rs
    build.rs
    cxx/
```

## 20. Public API sketch

```rust
pub struct RiveEngine;

impl RiveEngine {
    pub fn load(bytes: &[u8], opts: LoadOptions) -> Result<Arc<RiveFile>>;
    pub fn instantiate(file: Arc<RiveFile>, opts: InstanceOptions) -> Result<RiveHandle>;
}

pub struct RiveHandle { /* opaque */ }

impl RiveHandle {
    pub fn set_input(&self, name: &str, value: InputValue) -> Result<()>;
    pub fn play(&self) -> Result<()>;
    pub fn pause(&self) -> Result<()>;
    pub fn stop(&self) -> Result<()>;
    pub fn advance(&self, dt_seconds: f32) -> Result<AdvanceResult>;
    pub fn snapshot(&self) -> DisplayList;
}
```

## 21. Key design decisions

The biggest decision is this:

**Do not make WGPUI understand Rive directly.**
Make `oa_rive` emit a clean retained display list and host it as one widget.

That preserves replaceability. Later, if you abandon `.riv`, your app code still talks to the same animation widget API.

## 22. Honest risk assessment

The main technical risk is **format and semantic drift**. Since the official public surface emphasizes runtimes rather than a stable, third-party binary spec, a full clean-room parser is more fragile than a renderer-backend integration. ([Rive][2])

The second risk is **text and advanced effect fidelity**. Rive explicitly says different renderers have differed in supported features and fidelity across platforms. ([Rive][3])

The third risk is **scope creep**. If you chase complete editor parity, this becomes a product-sized effort.

# Recommendation

Build this in two steps:

1. **Spec the host-facing runtime exactly as above.**
2. **Implement semantics via the open runtime first, with your own WGPU backend.**

That gets you real `.riv` support inside WGPUI fastest, while keeping the door open to a future all-Rust port.

If you want, I can turn this into an **ADR-style repo document** next, with concrete Rust trait definitions and a milestone-by-milestone implementation plan.

[1]: https://github.com/rive-app/rive-runtime "GitHub - rive-app/rive-runtime: Low-level C++ Rive runtime and renderer · GitHub"
[2]: https://rive.app/runtimes?utm_source=chatgpt.com "Rive Runtimes"
[3]: https://rive.app/renderer "Rive Renderer"
[4]: https://help.rive.app/runtimes/state-machines "State Machine Playback - Rive"
[5]: https://help.rive.app/runtimes/loading-assets "Loading Assets - Rive"
