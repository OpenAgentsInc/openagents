# Own All Six: Implementation Plan for Coder

## Executive Summary

Implement the "own all six layers" UI stack for Coder (OpenAgents' coding agent platform), building a Rust-native GPU-accelerated UI system that delivers IDE-class interaction quality across web, desktop, and mobile.

## Key Decisions (Confirmed)

- **First vertical slice**: Chat thread - build on existing markdown streaming
- **Platform strategy**: Web + Desktop in parallel from day one
- **Reactive runtime**: Custom signals/effects (Solid.js-inspired)
- **Domain model**: Full event-sourced from day one (DomainEvent stream + projections)

---

## Crate Organization

### New Crates to Create

| Crate | Layer | Purpose |
|-------|-------|---------|
| `crates/coder_domain/` | 1 | Domain entities, events, projections |
| `crates/coder_protocol/` | 1 | Wire types for client/server sync |
| `crates/coder_ui_runtime/` | 2 | Signals, effects, scheduler, command bus |
| `crates/coder_widgets/` | 4 | Widget trait and core widgets |
| `crates/coder_surfaces_chat/` | 4 | Chat thread surface |
| `crates/coder_surfaces_terminal/` | 4 | Terminal emulator surface |
| `crates/coder_surfaces_diff/` | 4 | Diff viewer surface |
| `crates/coder_surfaces_timeline/` | 4 | Run timeline surface |

### Existing Crates to Modify

| Crate | Layer | Changes |
|-------|-------|---------|
| `crates/wgpui/` | 3,5,6 | Add input handling, clip stack, hit testing, desktop platform |
| `crates/dioxus/` | Shell | Add WgpuiCanvas component, integrate surfaces |

### Crate Dependency Graph

```
                          ┌─────────────────┐
                          │  coder_domain   │ ← Layer 1
                          └────────┬────────┘
                                   │
                          ┌────────▼────────┐
                          │ coder_protocol  │ ← Layer 1
                          └────────┬────────┘
                                   │
┌──────────────────────────────────┼──────────────────────────────────┐
│                                  │                                   │
│  ┌───────────────────┐   ┌──────▼──────┐   ┌───────────────────┐   │
│  │      wgpui        │◄──│coder_ui_    │──►│  coder_widgets    │   │
│  │ (render/layout/   │   │  runtime    │   │  (widget trait)   │   │
│  │  platform)        │   │ (signals)   │   └─────────┬─────────┘   │
│  └───────────────────┘   └─────────────┘             │             │
│          ▲                                           │             │
│          │                    ┌──────────────────────┼─────────────┤
│          │                    ▼                      ▼             │
│          │           ┌────────────────┐    ┌─────────────────┐     │
│          └───────────│coder_surfaces_ │    │ coder_surfaces_ │     │
│                      │     chat       │    │    terminal     │     │
│                      └────────────────┘    └─────────────────┘     │
│                                                                     │
│                      ┌────────────────┐    ┌─────────────────┐     │
│                      │coder_surfaces_ │    │ coder_surfaces_ │     │
│                      │     diff       │    │    timeline     │     │
│                      └────────────────┘    └─────────────────┘     │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                          ┌────────▼────────┐
                          │     dioxus      │ ← App shell
                          │ (WgpuiCanvas)   │
                          └─────────────────┘
```

### Module Structure Within wgpui

```
crates/wgpui/src/
├── lib.rs              # Public API
├── color.rs            # Hsla color (existing)
├── geometry.rs         # Point, Size, Bounds (existing)
├── theme.rs            # Design tokens (existing)
├── scene.rs            # Primitives (existing) + clip stack
├── layout.rs           # Taffy integration (existing)
├── text.rs             # cosmic-text (existing)
├── renderer.rs         # wgpu pipelines (existing)
├── input.rs            # NEW: Input events, keyboard state
├── hit_test.rs         # NEW: Hit testing
├── scroll.rs           # NEW: Scroll containers
├── markdown/           # Markdown rendering (existing)
├── shaders/            # WGSL shaders (existing)
└── platform/
    ├── mod.rs          # Platform trait (NEW)
    ├── web.rs          # Web platform (existing, enhanced)
    └── desktop.rs      # NEW: winit integration
```

---

## Current State Analysis

### What Exists (wgpui)

| Component | Status | Notes |
|-----------|--------|-------|
| GPU Rendering | Working | wgpu quad/text pipelines, SDF corners/borders |
| Text System | Working | cosmic-text shaping, glyph atlas |
| Layout Engine | Working | Taffy 0.9 flexbox integration |
| Theme System | Working | Bloomberg-dark tokens |
| Markdown | Working | Streaming, syntax highlighting, remend |
| Web Platform | Working | WASM, canvas init, events, resize |
| Native Platform | Missing | No winit integration |
| Mobile Platform | Missing | No iOS/Android |

### What Exists (Dioxus Crate)

- `CoderScreen` - Coder IDE shell
- `MechaCoder` - Chat interface with conversation graph
- Basic routing between views
- Tailwind CSS styling

### Layer Mapping to "Own All Six"

| Layer | Spec Name | Current State |
|-------|-----------|---------------|
| 1 | Domain Model | Not started |
| 2 | UI Runtime | Not started (no signals/effects) |
| 3 | Layout Engine | Partial (Taffy works, no virtual scrolling) |
| 4 | Widgets | Not started |
| 5 | Renderer | Partial (rendering works, missing clip stack/hit test) |
| 6 | Platform Glue | Partial (web only) |

---

## Strategic Approach

### Hybrid Architecture (Recommended)

Dioxus for the shell (auth, routing, forms, HTML inputs for IME) + wgpui canvas surfaces for performance-critical views.

```
┌─────────────────────────────────────────────────────────────┐
│  Dioxus Shell (SSR + Hydration)                              │
│  ├── Auth, settings, project list                           │
│  └── HTML inputs (IME support)                              │
├─────────────────────────────────────────────────────────────┤
│  wgpui Canvas Surfaces                                       │
│  ├── Chat thread (markdown streaming)                       │
│  ├── Terminal emulator                                      │
│  ├── Diff viewer                                            │
│  └── Run timeline                                           │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Order (Dependency-Driven)

```
Phase 1: Platform Parity + Input Foundation
    │
    ├── Desktop platform (winit integration)
    ├── Unified input handling (web + desktop)
    ├── Clip stack and scroll containers
    └── Hit testing with node IDs
    │
Phase 2: Domain Model (Event-Sourced from Day One)
    │
    ├── DomainEvent enum (append-only)
    ├── Core entities (Project, Workflow, Run, Artifact)
    ├── Projections (ChatView, TerminalView, etc.)
    └── Protocol types for sync
    │
Phase 3: Reactive Runtime + Command Bus
    │
    ├── Signal<T>, Memo<T>, Effect
    ├── Scope and cleanup
    ├── Frame scheduler (update → build → layout → paint)
    └── Command bus (UI intent → side effects)
    │
Phase 4: Widget System + Chat Surface
    │
    ├── Widget trait
    ├── Basic widgets (Div, Text, ScrollView)
    ├── Virtual scrolling
    └── Chat thread surface (first vertical slice)
    │
Phase 5: Additional IDE Surfaces
    │
    ├── Terminal surface
    ├── Diff viewer surface
    └── Run timeline surface
    │
Phase 6: Production Polish
    │
    ├── Accessibility (semantics tree)
    ├── IME integration
    └── Mobile platforms
```

---

## Phase 1: Platform Parity + Input Foundation

### 1.1 Desktop Platform (winit)

**Goal**: Run wgpui natively on macOS/Windows/Linux alongside web.

**Files to create**:
- `crates/wgpui/src/platform/desktop.rs` - winit + wgpu surface integration
- `crates/wgpui/src/platform/mod.rs` - Platform trait abstraction

**Dependencies to add**:
```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
winit = "0.30"
pollster = "0.3"  # For blocking async init on native
```

**Platform Trait**:
```rust
pub trait Platform {
    fn logical_size(&self) -> Size;
    fn scale_factor(&self) -> f32;
    fn request_redraw(&self);
    fn text_system(&mut self) -> &mut TextSystem;
    fn render(&mut self, scene: &Scene) -> Result<(), String>;
}
```

### 1.2 Input Handling

**Goal**: Handle all input events in wgpui canvas.

**Files to create/modify**:
- `crates/wgpui/src/input.rs` - Event types, keyboard state, modifiers
- `crates/wgpui/src/platform/web.rs` - Add keyboard, pointer, wheel listeners

**API**:
```rust
pub enum InputEvent {
    MouseDown { position: Point, button: MouseButton, modifiers: Modifiers },
    MouseUp { position: Point, button: MouseButton, modifiers: Modifiers },
    MouseMove { position: Point, modifiers: Modifiers },
    Wheel { delta: Point, modifiers: Modifiers },
    KeyDown { key: Key, code: KeyCode, modifiers: Modifiers, repeat: bool },
    KeyUp { key: Key, code: KeyCode, modifiers: Modifiers },
    TextInput { text: String },
}
```

### 1.2 Clip Stack & Scroll Containers

**Goal**: Support nested clipping and scroll containers.

**Files to create/modify**:
- `crates/wgpui/src/scene.rs` - Add `PushClip`, `PopClip` commands
- `crates/wgpui/src/renderer.rs` - Implement clip via scissor rects
- `crates/wgpui/src/scroll.rs` - ScrollContainer with bounds tracking

**API**:
```rust
impl Scene {
    pub fn push_clip(&mut self, bounds: Bounds);
    pub fn pop_clip(&mut self);
}

pub struct ScrollContainer {
    pub content_size: Size,
    pub viewport_size: Size,
    pub scroll_offset: Point,
}
```

### 1.3 Hit Testing

**Goal**: Map screen coordinates to scene nodes.

**Files to create/modify**:
- `crates/wgpui/src/hit_test.rs` - HitTestIndex, spatial lookup

**API**:
```rust
pub struct HitTestIndex {
    entries: Vec<HitTestEntry>,
}

pub struct Hit {
    pub node_id: u64,
    pub local_point: Point,
}

impl HitTestIndex {
    pub fn hit_test(&self, point: Point) -> Option<Hit>;
}
```

---

## Phase 2: Domain Model (Event-Sourced)

### 2.1 Core Entities

**Goal**: Define the domain primitives that drive UI state.

**Files to create**:
- `crates/coder_domain/Cargo.toml`
- `crates/coder_domain/src/lib.rs`
- `crates/coder_domain/src/project.rs`
- `crates/coder_domain/src/workflow.rs`
- `crates/coder_domain/src/run.rs`
- `crates/coder_domain/src/artifact.rs`
- `crates/coder_domain/src/message.rs`

**Core Types**:
```rust
// project.rs
pub struct Project {
    pub id: ProjectId,
    pub repo: RepoRef,
    pub env: Environment,
    pub secrets: SecretScope,
}

// workflow.rs
pub struct WorkflowSpec {
    pub id: WorkflowId,
    pub name: String,
    pub triggers: Vec<Trigger>,
    pub policies: Policies,
    pub steps: Vec<StepSpec>,
}

// run.rs
pub struct Run {
    pub id: RunId,
    pub workflow_id: WorkflowId,
    pub status: RunStatus,
    pub step_runs: Vec<StepRun>,
    pub cost: CostSummary,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
}

// message.rs (for chat)
pub struct Message {
    pub id: MessageId,
    pub role: Role,
    pub content: String,
    pub tool_uses: Vec<ToolUse>,
    pub timestamp: Timestamp,
}
```

### 2.2 Domain Events

**Goal**: Append-only event stream for all state changes.

**Files to create**:
- `crates/coder_domain/src/event.rs`

**Event Enum**:
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DomainEvent {
    // Project events
    ProjectCreated { project_id: ProjectId, repo: RepoRef },
    ProjectUpdated { project_id: ProjectId, changes: ProjectChanges },

    // Workflow events
    WorkflowSaved { workflow_id: WorkflowId, spec: WorkflowSpec },

    // Run events
    RunStarted { run_id: RunId, workflow_id: WorkflowId },
    RunStepUpdated { run_id: RunId, step_id: StepId, status: StepStatus, log_delta: String },
    RunArtifactAdded { run_id: RunId, artifact: Artifact },
    RunFinished { run_id: RunId, status: RunStatus, cost: CostSummary },

    // Chat events (for chat thread)
    MessageAdded { thread_id: ThreadId, message: Message },
    MessageStreaming { thread_id: ThreadId, message_id: MessageId, delta: String },
    MessageComplete { thread_id: ThreadId, message_id: MessageId },
    ToolUseStarted { thread_id: ThreadId, message_id: MessageId, tool_use: ToolUse },
    ToolUseComplete { thread_id: ThreadId, message_id: MessageId, tool_use_id: ToolUseId, result: ToolResult },
}
```

### 2.3 Projections

**Goal**: Derived views optimized for UI consumption.

**Files to create**:
- `crates/coder_domain/src/projections/mod.rs`
- `crates/coder_domain/src/projections/chat_view.rs`
- `crates/coder_domain/src/projections/run_timeline_view.rs`
- `crates/coder_domain/src/projections/terminal_view.rs`

**Chat Projection**:
```rust
pub struct ChatView {
    pub thread_id: ThreadId,
    pub entries: Vec<ChatEntry>,
    pub streaming_message: Option<StreamingMessage>,
}

pub enum ChatEntry {
    Message(MessageView),
    ToolUse(ToolUseView),
}

pub struct MessageView {
    pub id: MessageId,
    pub role: Role,
    pub content: String,  // Markdown
    pub timestamp: Timestamp,
}

pub struct StreamingMessage {
    pub id: MessageId,
    pub content_so_far: String,
    pub is_complete: bool,
}
```

### 2.4 Protocol Types

**Goal**: Wire types for client/server sync + streaming.

**Files to create**:
- `crates/coder_protocol/Cargo.toml`
- `crates/coder_protocol/src/lib.rs`
- `crates/coder_protocol/src/messages.rs`
- `crates/coder_protocol/src/streaming.rs`

**Protocol Messages**:
```rust
// Client → Server
pub enum ClientMessage {
    Subscribe { thread_id: ThreadId },
    Unsubscribe { thread_id: ThreadId },
    SendMessage { thread_id: ThreadId, content: String },
    CancelRun { run_id: RunId },
}

// Server → Client
pub enum ServerMessage {
    Events { events: Vec<DomainEvent> },
    Snapshot { thread: ChatView },
    Error { code: ErrorCode, message: String },
}
```

---

## Phase 3: Reactive Runtime + Command Bus

### 3.1 Reactive Runtime

**Goal**: Fine-grained reactivity for UI state (Solid.js-inspired).

**Files to create**:
- `crates/coder_ui_runtime/Cargo.toml`
- `crates/coder_ui_runtime/src/lib.rs`
- `crates/coder_ui_runtime/src/signal.rs` - `Signal<T>`, `Memo<T>`
- `crates/coder_ui_runtime/src/effect.rs` - `Effect`, side effect scheduling
- `crates/coder_ui_runtime/src/scope.rs` - Reactive scopes and cleanup
- `crates/coder_ui_runtime/src/scheduler.rs` - Frame phases: update, build, layout, paint

**Core Primitives**:
```rust
// signal.rs
pub struct Signal<T> {
    value: Rc<RefCell<T>>,
    subscribers: Rc<RefCell<Vec<EffectId>>>,
}

impl<T: Clone> Signal<T> {
    pub fn get(&self) -> T;          // Tracks dependency
    pub fn get_untracked(&self) -> T; // No tracking
    pub fn set(&self, value: T);     // Notifies subscribers
    pub fn update(&self, f: impl FnOnce(&mut T));
}

// memo.rs
pub struct Memo<T> {
    compute: Box<dyn Fn() -> T>,
    cached: RefCell<Option<T>>,
}

impl<T: Clone> Memo<T> {
    pub fn get(&self) -> T;  // Recomputes if dependencies changed
}

// effect.rs
pub fn effect(f: impl FnMut() + 'static) -> EffectHandle;
pub fn create_effect<T>(f: impl Fn() -> T + 'static) -> Memo<T>;
```

### 3.2 Frame Scheduler

**Goal**: Deterministic phase-based frame processing.

**Scheduler Phases**:
```rust
pub struct Scheduler {
    phase: Phase,
    pending_effects: Vec<EffectId>,
    dirty_nodes: Vec<NodeId>,
}

pub enum Phase {
    Update,   // Process signals, run effects
    Build,    // Update widget tree
    Layout,   // Compute positions
    Paint,    // Build display list
    Render,   // GPU submit
}

impl Scheduler {
    pub fn run_frame(&mut self) {
        self.update();   // Settle reactive graph
        self.build();    // Rebuild dirty widgets
        self.layout();   // Recompute layout
        self.paint();    // Generate scene
        self.render();   // Submit to GPU
    }
}
```

### 3.3 Command Bus

**Goal**: Separate UI intent from side effects.

**Files to create**:
- `crates/coder_ui_runtime/src/command.rs`

**API**:
```rust
pub enum Command {
    // Platform commands
    CopyToClipboard(String),
    OpenUrl(String),
    SetCursor(Cursor),

    // Navigation
    NavigateTo(Route),

    // Domain commands
    SendMessage { thread_id: ThreadId, content: String },
    CancelRun { run_id: RunId },
    ApproveStep { run_id: RunId, step_id: StepId },
}

pub trait CommandHandler {
    fn execute(&mut self, command: Command) -> CommandResult;
}
```

---

## Phase 4: Widget System + Chat Surface

### 4.1 Widget Trait

**Goal**: Composable UI building blocks.

**Files to create**:
- `crates/coder_widgets/Cargo.toml`
- `crates/coder_widgets/src/lib.rs`
- `crates/coder_widgets/src/widget.rs`
- `crates/coder_widgets/src/context.rs`

**Widget API**:
```rust
pub trait Widget: 'static {
    type State: Default;

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::State);
    fn paint(&mut self, bounds: Bounds, state: &mut Self::State, cx: &mut PaintContext);
    fn handle_event(&mut self, event: &InputEvent, cx: &mut EventContext) -> EventResult;

    // Optional
    fn semantics(&self) -> Option<SemanticsNode> { None }
}

pub enum EventResult {
    Handled,
    Ignored,
}
```

### 4.2 Core Widgets

**Files to create**:
- `crates/coder_widgets/src/div.rs` - Container widget
- `crates/coder_widgets/src/text.rs` - Text widget
- `crates/coder_widgets/src/scroll.rs` - ScrollView widget
- `crates/coder_widgets/src/list.rs` - Virtual list widget

**Virtual Scrolling**:
```rust
pub struct VirtualList<T, F: Fn(&T) -> impl Widget> {
    items: Signal<Vec<T>>,
    render_item: F,
    item_height: f32,
    scroll_offset: Signal<f32>,
}

impl<T, F> VirtualList<T, F> {
    fn visible_range(&self, viewport_height: f32) -> Range<usize>;
}
```

### 4.3 Chat Thread Surface (First Vertical Slice)

**Goal**: Replace MechaCoder's HTML chat thread with wgpui canvas.

**Files to create**:
- `crates/coder_surfaces_chat/Cargo.toml`
- `crates/coder_surfaces_chat/src/lib.rs`
- `crates/coder_surfaces_chat/src/thread.rs` - ChatThread widget
- `crates/coder_surfaces_chat/src/message.rs` - MessageBubble widget
- `crates/coder_surfaces_chat/src/tool_use.rs` - ToolUseIndicator widget

**Chat Thread Widget**:
```rust
pub struct ChatThread {
    chat_view: Signal<ChatView>,
    scroll_offset: Signal<f32>,
    selected_entry: Signal<Option<ChatEntryId>>,
}

impl Widget for ChatThread {
    // Virtual scrolling over ChatEntry items
    // Markdown rendering for message content
    // Streaming text support
}
```

**Integration with Dioxus**:
```rust
// crates/dioxus/src/components/wgpui_canvas.rs
#[component]
pub fn ChatCanvas(chat_view: Signal<ChatView>) -> Element {
    let platform = use_signal(|| None::<Platform>);

    use_effect(move || {
        // Initialize wgpui platform
        // Create ChatThread widget
        // Run frame loop
    });

    rsx! {
        canvas { id: "chat-canvas", style: "width: 100%; height: 100%;" }
    }
}
```

---

## Phase 5: Additional IDE Surfaces

### 5.1 Terminal Surface

**Goal**: ANSI-capable terminal emulator rendered via wgpui.

**Files to create**:
- `crates/coder_surfaces_terminal/Cargo.toml`
- `crates/coder_surfaces_terminal/src/lib.rs`
- `crates/coder_surfaces_terminal/src/ansi.rs` - ANSI parsing (using `vte` crate)
- `crates/coder_surfaces_terminal/src/buffer.rs` - Scrollback buffer
- `crates/coder_surfaces_terminal/src/terminal.rs` - Terminal widget

**Dependencies**:
```toml
vte = "0.13"  # ANSI escape sequence parser
```

**Features**:
- ANSI escape sequence parsing (colors, cursor, clear)
- Scrollback buffer with virtual scrolling
- Text selection and copy
- Cursor rendering (block, bar, underline)

### 5.2 Diff Viewer Surface

**Goal**: Side-by-side and inline diff rendering.

**Files to create**:
- `crates/coder_surfaces_diff/Cargo.toml`
- `crates/coder_surfaces_diff/src/lib.rs`
- `crates/coder_surfaces_diff/src/diff.rs` - Diff computation
- `crates/coder_surfaces_diff/src/view.rs` - DiffViewer widget
- `crates/coder_surfaces_diff/src/gutter.rs` - Line number gutter

**Features**:
- Side-by-side and unified views
- Line number gutters
- Change highlighting (add/remove/modify)
- Inline annotations from agent reviews

### 5.3 Run Timeline Surface

**Goal**: Visualize agent workflow execution.

**Files to create**:
- `crates/coder_surfaces_timeline/Cargo.toml`
- `crates/coder_surfaces_timeline/src/lib.rs`
- `crates/coder_surfaces_timeline/src/timeline.rs` - Timeline widget
- `crates/coder_surfaces_timeline/src/lane.rs` - Parallel execution lanes
- `crates/coder_surfaces_timeline/src/step.rs` - Step detail view

**Features**:
- Horizontal timeline with step blocks
- Parallel lanes for concurrent agents
- Streaming status updates
- Click to expand step details/artifacts

---

## Phase 6: Production Polish

### 6.1 Accessibility

**Files to create**:
- `crates/wgpui/src/semantics.rs` - Semantics tree types
- `crates/wgpui/src/platform/a11y_web.rs` - ARIA/DOM mirror
- `crates/wgpui/src/platform/a11y_desktop.rs` - Native a11y APIs

**Features**:
- Semantics tree generation from widget tree
- Web: ARIA attributes via hidden DOM mirror
- Desktop: Platform accessibility APIs (AX/UIA/AT-SPI)

### 6.2 IME Integration

**Files to modify**:
- `crates/wgpui/src/platform/web.rs` - Hidden textarea for composition
- `crates/wgpui/src/platform/desktop.rs` - Native IME hooks

**Features**:
- Web: Hidden textarea capture for CJK input
- Desktop: Native IME hooks via winit
- Composition preview rendering

### 6.3 Mobile Platforms

**Files to create**:
- `crates/wgpui/src/platform/ios.rs` - Metal surface
- `crates/wgpui/src/platform/android.rs` - Vulkan surface

---

## First Milestone: Chat Surface End-to-End

**Goal**: Replace MechaCoder's HTML chat thread with wgpui canvas, running on both web and desktop.

### Deliverables

1. **wgpui enhancements** (Phase 1)
   - Input event handling (keyboard, mouse, wheel)
   - Scroll container with clip stack
   - Hit testing with node IDs
   - Desktop platform via winit

2. **Domain model** (Phase 2)
   - `coder_domain` crate with Message, ChatView types
   - DomainEvent for streaming messages
   - ChatView projection

3. **Reactive runtime** (Phase 3)
   - `coder_ui_runtime` crate with Signal, Memo, Effect
   - Frame scheduler

4. **Chat surface** (Phase 4)
   - `coder_surfaces_chat` crate
   - ChatThread widget with virtual scrolling
   - MessageBubble widget with markdown rendering
   - Integration with existing streaming markdown

5. **Dioxus integration**
   - WgpuiCanvas component
   - Signal bridge from Dioxus to wgpui

### Success Criteria

- [ ] 60fps scrolling with 500+ messages
- [ ] Streaming text renders smoothly (no flicker)
- [ ] Works identically on web and desktop
- [ ] Markdown rendering matches current wgpui demo
- [ ] Selection and copy-to-clipboard work

---

## Summary: Files by Phase

### Phase 1 (Platform + Input)
```
MODIFY: crates/wgpui/Cargo.toml
MODIFY: crates/wgpui/src/scene.rs
MODIFY: crates/wgpui/src/renderer.rs
MODIFY: crates/wgpui/src/platform/web.rs
CREATE: crates/wgpui/src/input.rs
CREATE: crates/wgpui/src/hit_test.rs
CREATE: crates/wgpui/src/scroll.rs
CREATE: crates/wgpui/src/platform/mod.rs
CREATE: crates/wgpui/src/platform/desktop.rs
```

### Phase 2 (Domain Model)
```
CREATE: crates/coder_domain/Cargo.toml
CREATE: crates/coder_domain/src/lib.rs
CREATE: crates/coder_domain/src/message.rs
CREATE: crates/coder_domain/src/event.rs
CREATE: crates/coder_domain/src/projections/chat_view.rs
CREATE: crates/coder_protocol/Cargo.toml
CREATE: crates/coder_protocol/src/lib.rs
```

### Phase 3 (Runtime)
```
CREATE: crates/coder_ui_runtime/Cargo.toml
CREATE: crates/coder_ui_runtime/src/lib.rs
CREATE: crates/coder_ui_runtime/src/signal.rs
CREATE: crates/coder_ui_runtime/src/effect.rs
CREATE: crates/coder_ui_runtime/src/scheduler.rs
CREATE: crates/coder_ui_runtime/src/command.rs
```

### Phase 4 (Widgets + Chat)
```
CREATE: crates/coder_widgets/Cargo.toml
CREATE: crates/coder_widgets/src/lib.rs
CREATE: crates/coder_widgets/src/widget.rs
CREATE: crates/coder_widgets/src/scroll.rs
CREATE: crates/coder_widgets/src/list.rs
CREATE: crates/coder_surfaces_chat/Cargo.toml
CREATE: crates/coder_surfaces_chat/src/lib.rs
CREATE: crates/coder_surfaces_chat/src/thread.rs
CREATE: crates/coder_surfaces_chat/src/message.rs
MODIFY: crates/dioxus/src/main.rs
CREATE: crates/dioxus/src/components/wgpui_canvas.rs
```

### Workspace Changes
```
MODIFY: Cargo.toml (add new workspace members)
```
