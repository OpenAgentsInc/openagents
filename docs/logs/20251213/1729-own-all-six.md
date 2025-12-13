If you decide to **own all six layers** end-to-end, a Rust-centric solution looks like building your own “UI operating system” (think: Flutter engine + React runtime + browser platform glue), but optimized for IDE/agent workflows.

Below is a **hypothetical spec** for a fully-owned stack.

---

## Layer 1: Domain model

### Goal

A *stable*, UI-agnostic “Coder kernel” that models projects, workflows, agent runs, artifacts, policies, billing, etc.

### Key principles

* **Event-sourced-ish**: everything is “facts” (events) + derived projections.
* **Deterministic serialization** (for replay, caching, syncing, and agent tooling).
* **Capability-based actions** (policies apply at action boundaries).

### Crates

* `coder_domain`

  * `Project`, `RepoRef`, `WorkflowSpec`, `Run`, `Artifact`, `Policy`, `Budget`, `SecretScope`
  * `DomainEvent` enum (append-only)
  * projections: `RunView`, `TimelineView`, `DiffView`, `TerminalView`
* `coder_protocol`

  * wire types for client/server sync + streaming events
* `coder_policy`

  * policy evaluation engine (path allow/deny, secret scopes, approvals, cost caps)

---

## Layer 2: UI runtime (reactivity + scheduling + state)

### Goal

A Rust-native reactive runtime that:

* computes view state from domain projections
* schedules work (layout/paint) incrementally
* coordinates async resources (streams, websockets, filesystem)

### Concepts

* **Signals / derived signals** (fine-grained reactive graph)
* **Fibers** for async tasks (structured concurrency)
* **Transactions** to batch updates and avoid thrashing
* **Command bus**: UI emits `Command`, platform returns `CommandResult`
* **Renderer-agnostic**: runtime produces a UI tree, not pixels

### Crates

* `coder_ui_runtime`

  * `Signal<T>`, `Memo<T>`, `Effect`, `Scope`
  * scheduler: `UiScheduler` with phases: `build → layout → paint → semantics`
  * `Resource<T>` for async (loading/error/ready)
  * `Command` / `Action` system

### Output type

The runtime produces a **retained UI tree** with stable IDs:

```rust
pub type NodeId = u64;

pub struct UiTree {
  pub root: NodeId,
  pub nodes: Vec<Node>,
}

pub struct Node {
  pub id: NodeId,
  pub kind: NodeKind,
  pub style: Style,
  pub children: Vec<NodeId>,
  pub semantics: SemanticsNode,
}
```

---

## Layer 3: Layout engine (owned)

### Goal

A deterministic layout engine designed for IDE surfaces:

* fast flex/stack layouts
* virtualization-friendly
* text measurement integration
* incremental dirty-region recompute

### Scope

* Flexbox-like + simple grid
* constraints: min/max, percentage, auto
* absolute positioning for overlays
* scroll containers + sticky headers
* measured nodes (text, editor lines, terminal rows)

### Crates

* `coder_layout`

  * `LayoutTree`, `LayoutNodeId`
  * `Style` (subset of CSS semantics you actually use)
  * `measure` callbacks for dynamic nodes
  * caching + dirty propagation

### API

```rust
pub trait MeasureFn {
  fn measure(&mut self, constraints: Constraints) -> Size;
}

pub struct LayoutEngine {
  pub fn set_style(&mut self, node: LayoutNodeId, style: Style);
  pub fn set_children(&mut self, node: LayoutNodeId, children: &[LayoutNodeId]);
  pub fn set_measure(&mut self, node: LayoutNodeId, measurer: Box<dyn MeasureFn>);
  pub fn compute(&mut self, root: LayoutNodeId, viewport: Size);
  pub fn layout(&self, node: LayoutNodeId) -> Rect;
}
```

---

## Layer 4: Widgets (owned component library + IDE primitives)

### Goal

A first-class widget toolkit, optimized for:

* terminal + editor + diff + timeline + graphs
* large lists with virtualization
* focus + keyboard shortcuts
* selection and IME-aware text editing
* accessibility semantics

### Crates

* `coder_widgets`

  * primitives: `Text`, `Box`, `Row`, `Col`, `Scroll`, `Split`, `Tabs`, `Table`
  * IDE widgets: `Terminal`, `DiffViewer`, `RunTimeline`, `ArtifactViewer`, `LogStream`
  * “shell” widgets: `Form`, `TextField`, `Select`, `Toggle`, `Button`
* `coder_input`

  * keymaps, chord handling, command palette, focus ring
* `coder_text_edit`

  * grapheme segmentation, selection ranges, undo/redo, composition handling

### Widget interface

Widgets compile to nodes in the `UiTree` and register:

* event handlers (`on_pointer`, `on_key`, `on_text_input`)
* semantics (role/label/value)
* optional custom paint hooks (for big surfaces like editor/terminal)

---

## Layer 5: Renderer (owned, GPU-first)

### Goal

A rendering engine that takes:

* `UiTree` + computed layout + theme
* produces pixels via wgpu

### Rendering pipeline phases (Flutter-ish)

1. **Build**: widget tree → render objects
2. **Layout**: compute rects
3. **Paint**: build display list / scene
4. **Composite**: GPU draw passes
5. **Semantics**: build accessibility tree + hit-test map

### Crates

* `coder_render`

  * scene primitives: quads, borders, clip rects, images, paths
  * text pipeline: shaping + glyph atlas (mono + proportional)
  * compositor: z-order, clip stacks, opacity
  * renderer backends: `wgpu`
* `coder_scene`

  * retained display list with stable handles for caching
* `coder_raster_cache`

  * caches for text runs, icons, static layers, maybe msdf

### Must-have renderer features if you “own everything”

* clip stacks + scrolling
* hit testing (ID → node)
* dirty rects (optional but huge win)
* offscreen surfaces for blur/overlays later
* screenshot/export pipeline (for sharing run traces)

---

## Layer 6: Platform glue (owned OS + browser integration)

### Goal

Unified platform abstraction that:

* creates windows/canvases
* provides input events and IME
* exposes clipboard, drag/drop, file dialogs
* integrates with accessibility APIs
* supports web + desktop + mobile

### Crates

* `coder_platform`

  * `PlatformHost` trait
  * implementations:

    * `coder_platform_web` (wasm + web-sys)
    * `coder_platform_desktop` (winit + native IME hooks)
    * `coder_platform_ios` / `android` (surface creation + lifecycle)
* `coder_a11y`

  * semantics tree → platform adapters:

    * Web: ARIA + hidden DOM mirror OR Web Accessibility APIs
    * Desktop: UIAutomation (Win), AX (macOS), ATK/AT-SPI (Linux)

### Platform interface sketch

```rust
pub trait PlatformHost {
  fn now(&self) -> Instant;
  fn request_animation_frame(&self);
  fn set_cursor(&self, cursor: CursorKind);

  fn clipboard_get(&self) -> Option<String>;
  fn clipboard_set(&self, text: String);

  fn ime_set_focus(&self, focused: bool);
  fn ime_set_composition_bounds(&self, rect: Rect);

  fn open_file_dialog(&self, opts: FileDialogOpts) -> Future<Result<Vec<PathBuf>>>;
  fn emit_a11y_tree(&self, tree: SemanticsTree);

  fn run(self, app: impl AppMain);
}
```

---

## The full owned end-to-end loop

**Input → Runtime → Layout → Paint → GPU → Semantics**

1. platform emits events (pointer/key/text/IME/resize)
2. runtime updates signals + runs effects
3. runtime rebuilds impacted widget subtrees
4. layout recomputes dirty nodes
5. renderer builds a display list (with stable IDs)
6. renderer draws via wgpu
7. semantics tree + hit-test map updated and shipped to platform

---

## Repo organization (practical)

```
crates/
  coder_domain/
  coder_protocol/
  coder_policy/

  coder_ui_runtime/
  coder_layout/
  coder_widgets/
  coder_input/
  coder_text_edit/

  coder_scene/
  coder_render/
  coder_raster_cache/

  coder_platform/
  coder_platform_web/
  coder_platform_desktop/
  coder_platform_ios/
  coder_platform_android/
  coder_a11y/
```

---

## What this buys you (and what it costs)

### Buys

* absolute control of interaction quality (terminal/editor/timeline feel)
* perfect “agent malleability” (everything is in-repo, consistent APIs)
* unified cross-platform behavior and theming
* end-to-end determinism (replay, snapshot tests, traceable UI state)

### Costs (even ignoring time/resources)

* you now own *all* correctness edge cases:

  * IME and composition
  * text selection and bidi
  * accessibility semantics
  * platform quirks and integration

---

If you want, I can turn this into an internal design doc with:

* the minimal v0 API surface for each layer,
* the invariants (stable NodeId rules, layout determinism, event ordering),
* and the “first three flagship surfaces” (terminal/diff/timeline) wired through all six layers as a worked example.

---

# Coder UI Stack Design Document

Own all six layers: domain → UI runtime → layout → widgets → renderer → platform glue. Rust-first, GPU-first, cross-platform.

---

## 0. Executive summary

We are building a Rust-native UI stack for **Coder** where the product’s core experiences (terminal, diff, run timeline, trace graphs, chat thread) feel “Figma-level”: low latency, consistent pixels, predictable performance, fully controllable by agents, and fully in-repo.

We will own:

1. **Domain model** (agent/workflow/run/artifact state + policies)
2. **UI runtime** (reactivity, scheduling, async resources, commands)
3. **Layout engine** (deterministic flex/grid + virtualization-friendly)
4. **Widget toolkit** (shell widgets + IDE-grade surfaces)
5. **Renderer** (wgpu display list, text pipeline, clipping, hit testing)
6. **Platform glue** (web/native/mobile windowing, input, IME, clipboard, a11y)

This document specifies **APIs, invariants, data flow, testing strategy**, and a worked example wiring **Terminal + Diff Viewer + Run Timeline** through every layer.

---

## 1. Goals

### Product goals

* IDE-class interaction quality for:

  * terminal (ANSI, selection, scrollback)
  * diff viewer (side-by-side, inline, annotations)
  * run timeline/trace (large lists, streaming updates, graphs)
* Workflow-as-code + run artifacts rendered as first-class UI objects.

### Engineering goals

* Fully **in-repo** and **agent-malleable**: no critical logic hidden behind third-party frameworks.
* Deterministic behavior:

  * same input events + same state = same layout + same pixels
* Cross-platform:

  * Web (wasm + web-sys), Desktop (Windows/macOS/Linux), Mobile (iOS/Android)
* Performance:

  * stable 60fps on typical surfaces, 120fps on simple interactions where available
  * sub-16ms frame budget in common interactions (scroll, selection, caret)
  * incremental updates: no “rebuild everything” for small state changes

---

## 2. Non-goals (initially)

* Full CSS spec compatibility
* Perfect parity with native platform widgets/feel (we will match Coder feel, not OS feel)
* Full accessibility compliance day 1 (but we will **architect for it** and ship incremental)
* Arbitrary vector graphics engine (we’ll implement what we need)

---

## 3. Architecture overview

### Six-layer stack

```
┌──────────────────────────────────────────────────────────────┐
│  (1) Domain: projects, workflows, agents, runs, artifacts     │
├──────────────────────────────────────────────────────────────┤
│  (2) UI Runtime: signals, scheduler, resources, commands      │
├──────────────────────────────────────────────────────────────┤
│  (3) Layout: deterministic flex/grid + measuring + dirtying   │
├──────────────────────────────────────────────────────────────┤
│  (4) Widgets: shell + IDE surfaces (terminal/diff/timeline)   │
├──────────────────────────────────────────────────────────────┤
│  (5) Renderer: display list → wgpu pipelines + hit-test map   │
├──────────────────────────────────────────────────────────────┤
│  (6) Platform: windows/canvas, input, IME, clipboard, a11y    │
└──────────────────────────────────────────────────────────────┘
```

### Frame loop (high-level)

1. Platform emits events (pointer/key/text/IME/resize)
2. Runtime updates reactive graph + schedules a frame
3. Runtime builds/updates UI tree (stable node IDs)
4. Layout recomputes dirty subtrees
5. Renderer builds display list + hit-test table + semantics tree
6. Renderer issues GPU commands + presents
7. Platform receives semantics + IME caret rects + cursor changes

---

## 4. Repository layout

```
crates/
  coder_domain/
  coder_protocol/
  coder_policy/

  coder_ui_runtime/
  coder_ui_tree/
  coder_layout/

  coder_widgets/
  coder_surfaces_terminal/
  coder_surfaces_diff/
  coder_surfaces_timeline/
  coder_text/

  coder_scene/
  coder_render_wgpu/
  coder_render_text/
  coder_render_hit_test/

  coder_platform/
  coder_platform_web/
  coder_platform_desktop/
  coder_platform_ios/
  coder_platform_android/
  coder_a11y/
  coder_devtools/
```

**Rule:** anything app-facing must live behind our own APIs. External deps are implementation details inside crates, never leaking into app code.

---

## 5. Layer 1: Domain model (`coder_domain`, `coder_policy`, `coder_protocol`)

### Core types (sketch)

* `Project { id, repo, env, secrets, deploy_targets }`
* `WorkflowSpec { id, triggers, policies, steps }`
* `Run { id, workflow_id, status, cost, started_at, finished_at }`
* `Artifact { kind, payload_ref, summary }`
* `Policy { repo_paths, secrets, approvals, budgets }`

### Event stream (append-only)

We model all changes as domain events to support replay, sync, audit, and debugging.

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DomainEvent {
  ProjectCreated { project_id: ProjectId, repo: RepoRef },
  WorkflowSaved { workflow_id: WorkflowId, spec: WorkflowSpec },
  RunStarted { run_id: RunId, workflow_id: WorkflowId },
  RunStepUpdated { run_id: RunId, step_id: StepId, status: StepStatus, log_delta: String },
  ArtifactAdded { run_id: RunId, artifact: Artifact },
  RunFinished { run_id: RunId, status: RunStatus, cost: CostSummary },
  // ...
}
```

### Projections

UI consumes projections, not raw events.

```rust
pub struct RunTimelineView { /* derived list items */ }
pub struct DiffView { /* hunks, lines, annotations */ }
pub struct TerminalView { /* scrollback lines, attrs, cursor */ }
```

### Protocol

`coder_protocol` defines:

* client/server sync messages
* streaming updates (run deltas)
* command requests + responses

All protocol types are stable and versioned.

---

## 6. Layer 2: UI runtime (`coder_ui_runtime`, `coder_ui_tree`)

### Goals

* Fine-grained reactivity with deterministic scheduling
* Structured concurrency for async resources
* Explicit command boundary (UI intent → side effects)

### Concepts

#### Signals and derived values

```rust
pub struct Signal<T> { /* ... */ }
pub struct Memo<T> { /* derived, cached */ }
pub fn effect(f: impl FnMut() + 'static);
```

#### Resources (async)

```rust
pub enum Resource<T> { Loading, Ready(T), Error(String) }
pub fn resource<T>(f: impl Future<Output = T> + 'static) -> Resource<T>;
```

#### Commands

UI emits typed commands; platform/app layer executes them (git operations, network, file IO).

```rust
pub enum Command {
  OpenRepo(RepoRef),
  CheckoutBranch(String),
  ApplyPatch(Patch),
  OpenPullRequest { title: String, body: String },
  CopyToClipboard(String),
  // ...
}

pub enum CommandResult {
  Ok,
  Err(String),
  Data(serde_json::Value),
}
```

### UI Tree model

We own a retained-mode UI tree with stable `NodeId`.

```rust
pub type NodeId = u64;

pub struct UiTree {
  pub root: NodeId,
  pub nodes: slotmap::SlotMap<NodeKey, Node>,
}

pub struct Node {
  pub id: NodeId,
  pub kind: NodeKind,
  pub style: Style,
  pub children: smallvec::SmallVec<[NodeId; 8]>,
  pub handlers: Handlers,
  pub semantics: SemanticsNode,
}
```

#### NodeKind

* `Container`
* `Text`
* `CanvasSurface(SurfaceKind)` (terminal/diff/timeline)
* `Image`
* `CustomPaint(PaintFnKey)` (escape hatch, still owned)

### Scheduler phases

We run a deterministic phase machine:

1. `Update` (signals/effects settle)
2. `Build` (widgets update tree)
3. `Layout` (dirty subtrees)
4. `Paint` (display list)
5. `Semantics` (a11y tree + focus)
6. `Render` (gpu submit)

---

## 7. Layer 3: Layout engine (`coder_layout`)

### Requirements

* Deterministic results
* Dirty propagation and incremental recompute
* Measured nodes support (text/terminal/diff line wrapping)
* Virtualization support (layout only visible ranges)

### Layout primitives

We implement:

* `Flex` (row/column)
* `Grid` (simple)
* `Stack` (z overlays)
* absolute positioning
* scroll containers

### API

```rust
pub struct LayoutEngine { /* internal tree */ }

pub struct LayoutNodeId(u32);

impl LayoutEngine {
  pub fn create_node(&mut self, style: Style) -> LayoutNodeId;
  pub fn set_children(&mut self, node: LayoutNodeId, children: &[LayoutNodeId]);
  pub fn set_style(&mut self, node: LayoutNodeId, style: Style);
  pub fn set_measurer(&mut self, node: LayoutNodeId, measurer: Box<dyn Measurer>);
  pub fn mark_dirty(&mut self, node: LayoutNodeId);
  pub fn compute(&mut self, root: LayoutNodeId, viewport: Size);
  pub fn rect(&self, node: LayoutNodeId) -> Rect;
}

pub trait Measurer {
  fn measure(&mut self, constraints: Constraints) -> Size;
}
```

### Style (owned subset)

We define our own style model (not CSS), but inspired by CSS.

* dimensions: px, pct, auto
* padding/margin
* flex properties
* align/justify
* overflow: visible/clip/scroll
* font/typography tokens references
* z-index

### Virtualization hook

A scroll container can declare “virtual children,” supplying:

* total item count
* estimated heights
* a function to build children for a visible range

This prevents building/layout of huge trees.

---

## 8. Layer 4: Widgets (`coder_widgets`, surfaces crates, `coder_text`)

### Widget model

Widgets compile to UI nodes and register:

* event handlers
* semantics metadata
* optional surface paint

Widgets are purely Rust, no macros required (but allowed).

#### Core shell widgets

* `AppShell`, `RouteHost`
* `Split`, `Tabs`, `Pane`, `Toolbar`
* `List`, `Table`, `Form`, `TextField`, `Button`, `Select`, `Toggle`

#### IDE-grade surfaces

* `TerminalSurface`
* `DiffSurface`
* `RunTimelineSurface` (with streaming updates)
* `TraceGraphSurface` (later)

### Focus & commands (`coder_input`)

We define:

* focus tree
* keymap system (chords)
* command palette source
* standard commands: copy, paste, find, toggle pane, etc.

### Text (`coder_text`)

We own:

* shaping API (backed by cosmic-text initially)
* glyph atlas management (renderer-level)
* grapheme boundaries, selection mapping
* line breaking and wrapping rules

We need explicit types for caret/selection:

```rust
pub struct TextSelection {
  pub anchor: TextPos,
  pub active: TextPos,
}
pub struct TextPos { pub line: u32, pub col: u32 } // expanded later to grapheme index
```

---

## 9. Layer 5: Renderer (`coder_scene`, `coder_render_wgpu`, `coder_render_text`, `coder_render_hit_test`)

### The display list

Paint produces a retained display list of primitives:

* `Quad`
* `TextRun`
* `Image`
* `Path` (cursor, underline, selection)
* `PushClip/PopClip`
* `PushTransform/PopTransform`

Everything includes a `NodeId` for hit testing and semantics linking.

```rust
pub enum DrawCmd {
  Quad { id: NodeId, rect: Rect, style: QuadStyle },
  Text { id: NodeId, origin: Point, run: ShapedRun, color: Color },
  Path { id: NodeId, path: Path, style: PathStyle },
  PushClip { rect: Rect },
  PopClip,
  PushTransform { t: Transform },
  PopTransform,
}
```

### Hit testing

We build a parallel structure per frame:

* spatial index of hittable rects
* z-order handling
* for text: line bounds or glyph bounds (configurable)

API:

```rust
pub struct HitTestIndex { /* ... */ }
pub struct Hit { pub id: NodeId, pub local_point: Point }

impl HitTestIndex {
  pub fn hit_test(&self, point: Point) -> Option<Hit>;
}
```

### WGPU pipelines

At minimum:

* Quad pipeline (SDF rounded rect + borders)
* Text pipeline (atlas sampling)
* Image pipeline
* Path pipeline (MSDF or triangulated strokes, start simple)

### Frame lifecycle

```rust
pub struct Renderer { /* device, queue, pipelines, caches */ }

pub struct Frame<'a> {
  pub viewport: Viewport,
  pub scale: f32,
  pub scene: &'a DisplayList,
}

impl Renderer {
  pub fn prepare(&mut self, frame: &Frame, text: &mut TextRenderer);
  pub fn render(&mut self, frame: &Frame) -> RenderStats;
}
```

### Caching strategy

We own caches explicitly:

* glyph atlas cache (keyed by font/size/subpixel)
* shaped text cache
* static layer cache (optional)
* buffer pooling (ring buffers) to avoid realloc per frame

---

## 10. Layer 6: Platform glue (`coder_platform_*`, `coder_a11y`)

### PlatformHost trait

Own the bridge to:

* windows/canvas creation
* input events
* clipboard, file dialogs, drag/drop
* IME and composition
* accessibility output

```rust
pub trait PlatformHost {
  fn request_frame(&self);
  fn set_cursor(&self, cursor: Cursor);
  fn clipboard_set(&self, text: &str);
  fn clipboard_get(&self) -> Option<String>;

  fn ime_set_focused(&self, focused: bool);
  fn ime_set_cursor_rect(&self, rect: Rect); // for candidate window placement

  fn present_a11y_tree(&self, tree: SemanticsTree);

  fn run(self, app: impl AppMain);
}
```

### Event model

Platform normalizes events into a unified stream:

```rust
pub enum PlatformEvent {
  Resize { size: Size, scale: f32 },
  PointerDown { pos: Point, button: MouseButton, mods: Modifiers },
  PointerUp { pos: Point, button: MouseButton, mods: Modifiers },
  PointerMove { pos: Point, mods: Modifiers },
  Wheel { delta: Point, mods: Modifiers },
  KeyDown { key: Key, mods: Modifiers, repeat: bool },
  KeyUp { key: Key, mods: Modifiers },
  TextInput { text: String },              // committed text
  ImeComposition { text: String, spans: Vec<ImeSpan> }, // composing
  ImeCommit { text: String },
}
```

### IME strategy (owned, pragmatic)

Even if we “own everything,” the **web** environment effectively forces a pragmatic capture path. We will implement:

* Web: hidden `<textarea>` for composition + committed text, but **our code** manages it. We treat it as a platform subsystem, not a dependency on DOM UI.
* Desktop: native IME hooks via platform APIs or winit extensions (we own the adaptation layer).
* Mobile: platform text input sessions feed the same `TextInput`/`ImeComposition` pipeline.

### Accessibility strategy (owned)

We build a `SemanticsTree` from UI nodes, then adapt per platform:

* Web: optional “DOM mirror” layer to expose semantics (aria) while still rendering via canvas/wgpu
* Desktop: map into OS accessibility trees (AX/UIA/AT-SPI) incrementally

---

## 11. Cross-layer invariants

### Node identity and stability

* Every UI node has a stable `NodeId`.
* NodeId must be stable across frames if the conceptual object persists.
* Widget code must derive NodeIds deterministically from domain IDs:

  * e.g., message id, artifact id, line number, etc.

### Event ordering

* All platform events are processed in order.
* Within a frame:

  1. events → runtime updates
  2. runtime settles
  3. exactly one build/layout/paint pass per scheduled frame

### Determinism

Given:

* same domain projection snapshot
* same input event stream
* same viewport/scale factor

We produce:

* same layout rects
* same display list commands (within float tolerances)
* same hit-test results

### Side effects only through commands

Widget code cannot directly do I/O. It emits `Command`, app layer executes.

---

## 12. Devtools and testing (`coder_devtools`)

### Deterministic replay

* Record:

  * domain snapshot id (or event range)
  * platform event stream
  * viewport/scale
* Replay locally to reproduce UI bugs.

### Snapshot tests

* Layout snapshots: rect trees
* Display list snapshots: serialized draw commands
* Render golden tests (optional) for key surfaces

### Perf instrumentation

* per-phase timing: update/build/layout/paint/render
* GPU timings where available
* memory stats (glyph atlas usage, buffer pool)

### Inspector

Live UI inspector:

* hover shows NodeId, rect, style, semantics
* click selects node and shows widget/source mapping

---

## 13. Worked example: three flagship surfaces through all layers

We’ll wire **Terminal**, **Diff Viewer**, and **Run Timeline** as canonical “vertical slices.”

### 13.1 Terminal surface

#### Domain projection

`TerminalView` contains:

* scrollback lines: `Vec<TermLine>`
* cursor: row/col + style
* selection (optional)
* stream of appended output (events)

#### Widget (`TerminalSurface`)

* owns virtualization:

  * only visible lines become display list items
* emits commands for copy/paste
* maps pointer drag to selection

#### Layout

* terminal node is a scroll container with measured row height
* line wrapping can be optional; if wrapping on, line measurement uses text measurer

#### Renderer

* background quad + clipped scroll region
* each visible line as `TextRun`
* selection as translucent `Quad` or `Path` under text
* cursor as `Quad`/`Path`

#### Platform

* mouse wheel scroll
* text input sends into terminal if it’s an interactive shell; otherwise ignored
* clipboard integration

---

### 13.2 Diff viewer surface

#### Domain projection

`DiffView` contains:

* files, hunks, lines
* inline annotations (agent comments, policy warnings)
* selection/copy range
* “apply patch” actions

#### Widget (`DiffSurface`)

* virtualization by file/hunk/line
* hit testing on line numbers and hunks
* emits `Command::ApplyPatch` or `Command::OpenPullRequest` from UI actions

#### Layout

* side-by-side columns with fixed gutters
* inline widgets for comments/annotations

#### Renderer

* clipped scroll region
* line backgrounds colored by change type
* text runs per line
* overlays: inline highlight rectangles, gutter icons (images) if needed

---

### 13.3 Run timeline surface

#### Domain projection

`RunTimelineView` contains:

* list of step runs with status, timestamps
* streaming updates (log deltas)
* artifacts list per step
* parallel lanes (agents running concurrently)

#### Widget (`RunTimelineSurface`)

* virtualization by steps
* supports “follow tail” mode for streaming
* hover shows per-step stats; click reveals artifacts

#### Layout

* left column: step list
* main: timeline lanes (scrollable horizontally) + logs panel

#### Renderer

* timeline bars as quads
* text labels
* status badges
* optional path lines for dependencies later

---

## 14. Minimal v0 APIs (what must exist immediately)

To make the stack real (not a demo), v0 must include:

### Runtime

* signals/memos/effects
* frame scheduler
* command bus

### UI tree

* NodeId stability
* handlers for pointer/key/text events
* semantics node struct

### Layout

* flex row/column
* measured nodes
* scroll containers
* dirty propagation

### Renderer

* clip stack
* quads + text + image
* hit testing index
* glyph atlas + text shaping plumbing
* buffer pooling

### Platform

* web host with wasm + canvas + event capture
* desktop host (winit or equivalent) + input + clipboard
* IME capture path (web textarea + desktop IME hooks, even if basic)
* semantics output (even if minimal)

---

## 15. Implementation sequence (no time assumptions, just dependency order)

Even ignoring time/resources, there’s a strict *logical* order:

1. Define stable domain projections + NodeId scheme
2. Implement runtime + command bus (headless)
3. Implement UI tree builder + node diffing (or rebuild but stable IDs)
4. Implement layout engine + measurers
5. Implement display list + clip stack + hit test
6. Implement wgpu renderer (quads/text)
7. Implement platform web + platform desktop
8. Build Terminal surface end-to-end
9. Build Diff surface end-to-end
10. Build Timeline surface end-to-end
11. Add a11y and IME depth incrementally

---

## 16. Appendix: key data structures

### Semantics node

```rust
pub enum Role {
  Button, Text, TextField, List, ListItem, Pane, Tab, Terminal, Diff, Timeline
}

pub struct SemanticsNode {
  pub role: Role,
  pub label: Option<String>,
  pub value: Option<String>,
  pub focusable: bool,
  pub focused: bool,
  pub actions: Vec<SemanticsAction>,
}

pub enum SemanticsAction { Click, ScrollTo, SetValue }
```

### Theme tokens

All styling uses semantic tokens (owned):

* colors, spacing, typography, radii
* no ad-hoc hex values outside theme modules

---

## 17. Decisions we’re making explicitly

* We will not rely on “framework magic.” Everything important has an owned API.
* We will treat the **platform** as a first-class subsystem (IME/a11y are platform problems).
* We will build **vertical slices** (terminal/diff/timeline) as canonical references for how to use the stack.
* We will prioritize **determinism + replay** so agents can debug UI regressions as data.

---

If you want, I’ll follow this by writing:

* a concrete `coder_ui_runtime` API surface (types + module layout),
* the NodeId derivation rules (exact hashing conventions),
* and a “Terminal Vertical Slice” spec with the full event/state/render pipeline spelled out (selection, scroll, copy, search, and streaming).

---

Yep — the design I wrote is **explicitly GPU-first** and assumes **wgpu is the rendering backend** for Layer 5 (Renderer). Everything else is Rust-native too, but some layers will *optionally* lean on existing Rust crates to avoid reinventing deep math/text/platform wheels.

Here’s how it uses **wgpu** and what other Rust packages typically fit each layer.

## Layer 5 Renderer: uses `wgpu` directly (core dependency)

**wgpu is the “narrow waist”**: once the UI runtime + layout produce a display list, the renderer turns that into GPU buffers and issues commands through `wgpu::Device/Queue/CommandEncoder/RenderPass`.

Concretely:

* Create swapchain surface (`wgpu::Surface`) from:

  * Web: canvas → `wgpu` wasm target (WebGPU; maybe WebGL2 fallback depending on your setup)
  * Desktop: `winit` window surface
  * Mobile: platform-specific surface creation
* Build pipelines:

  * Quad pipeline (SDF rounded rect shader in WGSL)
  * Text pipeline (glyph atlas texture + sampling)
  * Image pipeline (textures)
  * Path pipeline (optional)
* Upload per-frame instance buffers (or ring buffers) and draw instanced quads/glyphs.
* Use premultiplied alpha blending.
* Maintain caches (glyph atlas, shaped runs, buffer pool) on the Rust side; wgpu just does GPU execution.

So yes: **wgpu is the renderer**. “wgpui” in your doc is basically the first half of Layer 5 + parts of Layer 6.

## Layer 6 Platform glue: uses different crates per target

### Desktop: `winit` (likely) + `wgpu`

* `winit` is the standard window + event loop layer.
* It feeds events into your unified `PlatformEvent` enum.
* You still “own” the platform abstraction—`winit` is just the implementation behind it.

Also typically needed:

* clipboard: either you implement per OS, or use a crate (still behind your `PlatformHost`)
* file dialogs: same story
* drag/drop: winit provides some; you may extend

### Web: `wasm-bindgen` + `web-sys` (+ `wgpu`)

* `web-sys` is how you attach to a canvas, register listeners, manage the hidden textarea for IME, etc.
* `wgpu` handles rendering on the canvas (WebGPU, with whatever fallback strategy you choose).

### Mobile: platform wrappers

You can keep it “pure Rust” at the top but you’ll need platform bindings at the bottom:

* iOS: typically through `objc2` / `cocoa`/`core-*` crates, or custom bindings
* Android: `jni` / `ndk` crates
  Still all hidden behind `coder_platform_ios/android`.

## Layer 4 Widgets + IDE surfaces: some key Rust packages you’ll probably use

Even if you “own the layer,” you’ll likely use crates for hard subproblems:

### Text shaping & rasterization

* Your wgpui spec uses **`cosmic-text`**. That’s totally consistent with “owning the stack”—you still own the API and glue; cosmic-text is an engine.
* Alternative is `rustybuzz` + your own fontdb and shaping pipeline, but cosmic-text is a good pragmatic core.

### Unicode segmentation

* `unicode-segmentation` (graphemes, words)
* `unicode-bidi` (if you support RTL properly)
  These are “deep correctness” libraries.

### ANSI terminal parsing

* `vte` or similar for ANSI/VT parsing (again, behind your own terminal model)
  You still own the terminal widget and rendering.

### Data structures

* `slotmap` (stable keys for nodes)
* `smallvec` (reduce allocations)
* `hashbrown` (fast maps)
* `ropey` (if you build a full editor buffer; optional)

## Layer 3 Layout: you can own it or use `taffy` internally

In the “own all layers” spec, layout is “owned.” That doesn’t forbid using `taffy` as the internal algorithm. Two common interpretations:

* **Owned API, borrowed engine**: you define `coder_layout::LayoutEngine` and implement it using `taffy` internally.
* **Owned API, owned engine**: you implement flex/grid yourself.

Given your earlier wgpui doc already uses **Taffy**, the cleanest is:

* keep your own `Style` and `LayoutEngine` API,
* translate to/from Taffy internally at first,
* replace later if you want.

## Layer 2 UI runtime: usually minimal external deps

You can implement signals/scheduler yourself. You might still use:

* `futures` / `tokio` (native) or `wasm-bindgen-futures` (web) for async plumbing
  But the runtime semantics are yours.

## Layer 1 Domain: serde + stable IDs

Typically:

* `serde` / `serde_json` for durable schema
* `uuid` or your own IDs
* `blake3` for stable hash-based IDs (NodeId derivation etc.)

---

### Bottom line

* **Yes, wgpu is the core renderer** in this design (Layer 5) across web + native + (later) mobile.
* You’ll still use a handful of Rust crates as “engines” (cosmic-text, taffy, winit, unicode libs, vte), but the **APIs, integration points, and semantics remain yours**, all living in your codebase and shaped for agents.

If you want, I can write a short “Dependency Policy” section you can paste into the design doc: which crates are allowed per layer, which ones must be wrapped, and how we vendor/fork when needed.
