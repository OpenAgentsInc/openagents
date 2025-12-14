# Own All Six: Implementation Plan for Coder

## Executive Summary

Implement the "own all six layers" UI stack for Coder (OpenAgents' coding agent platform), building a Rust-native GPU-accelerated UI system that delivers IDE-class interaction quality across web, desktop, and mobile.

**Key Architectural Decision: Dioxus is Replaced**

In the "own all six layers" world, **Dioxus has no role**. We are building what Dioxus *is* (a UI runtime + component model + renderer adapters), plus more (our own layout, renderer, platform glue). The stack we're building replaces Dioxus entirely.

## Key Decisions (Confirmed)

- **First vertical slice**: Chat thread - build on existing markdown streaming
- **Platform strategy**: Web + Desktop in parallel from day one
- **Reactive runtime**: Custom signals/effects (Solid.js-inspired)
- **Domain model**: Full event-sourced from day one (DomainEvent stream + projections)
- **No Dioxus**: We own all six layers - Dioxus is not used

---

## The Six Layers We Own

| Layer | What We Own | Replaces |
|-------|-------------|----------|
| 1 | **Domain Model** (`coder_domain`, `coder_protocol`) | N/A |
| 2 | **UI Runtime** (`coder_ui_runtime` - signals, effects, scheduler) | Dioxus reactivity |
| 3 | **Layout Engine** (`wgpui` - Taffy integration) | Dioxus layout |
| 4 | **Widgets** (`coder_widgets`, `coder_surfaces_*`) | Dioxus components |
| 5 | **Renderer** (`wgpui` - wgpu display list, text pipeline) | Dioxus renderers |
| 6 | **Platform Glue** (`wgpui` - web-sys/winit/mobile + IME/a11y) | Dioxus platform adapters |

---

## Crate Organization

### New Crates to Create

| Crate | Layer | Purpose |
|-------|-------|---------|
| `crates/coder_domain/` | 1 | Domain entities, events, projections |
| `crates/coder_protocol/` | 1 | Wire types for client/server sync |
| `crates/coder_ui_runtime/` | 2 | Signals, effects, scheduler, command bus |
| `crates/coder_widgets/` | 4 | Widget trait and core widgets |
| `crates/coder_shell/` | 4 | Application shell (routing, navigation, chrome) |
| `crates/coder_surfaces_chat/` | 4 | Chat thread surface |
| `crates/coder_surfaces_terminal/` | 4 | Terminal emulator surface |
| `crates/coder_surfaces_diff/` | 4 | Diff viewer surface |
| `crates/coder_surfaces_timeline/` | 4 | Run timeline surface |
| `crates/coder_app/` | - | Main application binary (replaces dioxus crate) |

### Existing Crates to Modify

| Crate | Layer | Changes |
|-------|-------|---------|
| `crates/wgpui/` | 3,5,6 | Add input handling, clip stack, hit testing, desktop platform |

### Crate to Remove (Eventually)

| Crate | Reason |
|-------|--------|
| `crates/dioxus/` | Replaced by our own stack |

### Crate Dependency Graph (No Dioxus)

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
│          ▲                      │                    │             │
│          │                      │    ┌───────────────┴─────────────┤
│          │                      ▼    ▼                             │
│          │              ┌─────────────────┐   ┌────────────────┐   │
│          │              │   coder_shell   │   │coder_surfaces_ │   │
│          │              │ (routing/chrome)│   │     chat       │   │
│          │              └────────┬────────┘   └────────────────┘   │
│          │                       │                                  │
│          │                       │            ┌────────────────┐   │
│          └───────────────────────┼────────────│coder_surfaces_ │   │
│                                  │            │    terminal    │   │
│                                  │            └────────────────┘   │
│                                  │                                  │
│                                  │            ┌────────────────┐   │
│                                  │            │coder_surfaces_ │   │
│                                  │            │     diff       │   │
│                                  │            └────────────────┘   │
└──────────────────────────────────┼──────────────────────────────────┘
                                   │
                          ┌────────▼────────┐
                          │   coder_app     │ ← Main binary
                          │  (entry point)  │
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
├── input.rs            # Input events, keyboard state
├── hit_test.rs         # Hit testing
├── scroll.rs           # Scroll containers
├── markdown/           # Markdown rendering (existing)
├── shaders/            # WGSL shaders (existing)
└── platform/
    ├── mod.rs          # Platform trait
    ├── web.rs          # Web platform (existing, enhanced)
    └── desktop.rs      # winit integration
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
| Markdown | Working | Streaming, syntax highlighting |
| Web Platform | Working | WASM, canvas init, events, resize |
| Native Platform | Partial | winit integration added (Phase 1) |
| Input Handling | Done | Phase 1 complete |
| Hit Testing | Done | Phase 1 complete |
| Scroll Containers | Done | Phase 1 complete |

### What We've Built So Far

| Crate | Status | Tests |
|-------|--------|-------|
| `coder_domain` | ✅ Complete | 12 tests |
| `coder_protocol` | ✅ Complete | 3 tests |
| `coder_ui_runtime` | ✅ Complete | 20 tests |
| `coder_widgets` | ✅ Complete | 8 tests |
| `coder_shell` | Not started | - |
| `coder_surfaces_chat` | Not started | - |
| `coder_app` | Not started | - |

### Layer Completion Status

| Layer | Status | Notes |
|-------|--------|-------|
| 1 | ✅ Complete | Domain model + protocol |
| 2 | ✅ Complete | Reactive runtime |
| 3 | ✅ Complete | Layout via Taffy |
| 4 | 🔄 In Progress | Widget trait done, surfaces pending |
| 5 | ✅ Complete | wgpu renderer |
| 6 | ✅ Complete | Web + Desktop platforms |

---

## Strategic Approach

### Full Ownership Architecture

We own everything. No Dioxus. No hybrid. One unified Rust stack.

```
┌─────────────────────────────────────────────────────────────┐
│  coder_app (entry point)                                     │
│  ├── Platform initialization (web/desktop)                  │
│  └── Application bootstrap                                   │
├─────────────────────────────────────────────────────────────┤
│  coder_shell                                                 │
│  ├── Router (URL ↔ View mapping)                            │
│  ├── Navigation (back/forward, deep links)                  │
│  ├── Chrome (window frame, status bar)                      │
│  └── Auth/Session management                                │
├─────────────────────────────────────────────────────────────┤
│  coder_surfaces_*                                            │
│  ├── Chat thread (markdown streaming)                       │
│  ├── Terminal emulator                                      │
│  ├── Diff viewer                                            │
│  └── Run timeline                                           │
├─────────────────────────────────────────────────────────────┤
│  coder_widgets                                               │
│  ├── Widget trait + AnyWidget                               │
│  ├── Div, Text, ScrollView, VirtualList                     │
│  └── Input widgets (TextInput, Button)                      │
├─────────────────────────────────────────────────────────────┤
│  coder_ui_runtime                                            │
│  ├── Signal<T>, Memo<T>, Effect                             │
│  ├── Scope management                                       │
│  ├── Frame scheduler                                        │
│  └── Command bus                                            │
├─────────────────────────────────────────────────────────────┤
│  wgpui                                                       │
│  ├── Layout (Taffy)                                         │
│  ├── Renderer (wgpu)                                        │
│  └── Platform (web-sys/winit)                               │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Order (Dependency-Driven)

```
Phase 1: Platform Parity + Input Foundation ✅ COMPLETE
    │
    ├── Desktop platform (winit integration)
    ├── Unified input handling (web + desktop)
    ├── Clip stack and scroll containers
    └── Hit testing with node IDs
    │
Phase 2: Domain Model (Event-Sourced) ✅ COMPLETE
    │
    ├── DomainEvent enum (append-only)
    ├── Core entities (Message, Run, ToolUse)
    ├── Projections (ChatView)
    └── Protocol types for sync
    │
Phase 3: Reactive Runtime + Command Bus ✅ COMPLETE
    │
    ├── Signal<T>, Memo<T>, Effect
    ├── Scope and cleanup
    ├── Frame scheduler (update → build → layout → paint)
    └── Command bus (UI intent → side effects)
    │
Phase 4: Widget System ✅ COMPLETE (core widgets)
    │
    ├── Widget trait
    ├── Basic widgets (Div, Text, ScrollView)
    └── Virtual scrolling (VirtualList)
    │
Phase 5: Application Shell + Chat Surface ← CURRENT
    │
    ├── coder_shell (routing, navigation, chrome)
    ├── coder_surfaces_chat (chat thread widget)
    ├── coder_app (main binary)
    └── Input widgets (TextInput for chat)
    │
Phase 6: Additional IDE Surfaces
    │
    ├── Terminal surface
    ├── Diff viewer surface
    └── Run timeline surface
    │
Phase 7: Production Polish
    │
    ├── Accessibility (semantics tree)
    ├── IME integration
    └── Mobile platforms
```

---

## Phase 5: Application Shell + Chat Surface

### 5.1 Application Shell (`coder_shell`)

**Goal**: Routing, navigation, window chrome - the "frame" around surfaces.

**Files to create**:
- `crates/coder_shell/Cargo.toml`
- `crates/coder_shell/src/lib.rs`
- `crates/coder_shell/src/router.rs` - URL ↔ View mapping
- `crates/coder_shell/src/navigation.rs` - History, back/forward
- `crates/coder_shell/src/chrome.rs` - Window chrome, status bar
- `crates/coder_shell/src/views.rs` - View enum and registry

**Router**:
```rust
pub enum Route {
    Chat { thread_id: ThreadId },
    Project { project_id: ProjectId },
    Settings,
    Home,
}

pub struct Router {
    current: Signal<Route>,
    history: Vec<Route>,
}

impl Router {
    pub fn navigate(&mut self, route: Route);
    pub fn back(&mut self);
    pub fn forward(&mut self);
}
```

### 5.2 Chat Surface (`coder_surfaces_chat`)

**Goal**: Full chat thread with markdown, streaming, tool use indicators.

**Files to create**:
- `crates/coder_surfaces_chat/Cargo.toml`
- `crates/coder_surfaces_chat/src/lib.rs`
- `crates/coder_surfaces_chat/src/thread.rs` - ChatThread widget
- `crates/coder_surfaces_chat/src/message.rs` - MessageBubble widget
- `crates/coder_surfaces_chat/src/tool_use.rs` - ToolUseIndicator widget
- `crates/coder_surfaces_chat/src/input.rs` - Chat input widget

**Chat Thread**:
```rust
pub struct ChatThread {
    chat_view: Signal<ChatView>,
    scroll: ScrollContainer,
}

impl Widget for ChatThread {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Virtual scroll over messages
        // Render markdown for each message
        // Show tool use indicators inline
        // Handle streaming message at bottom
    }
}
```

### 5.3 Input Widgets

**Goal**: Text input for chat, form fields.

**Files to create**:
- `crates/coder_widgets/src/text_input.rs` - Single-line text input
- `crates/coder_widgets/src/button.rs` - Clickable button

**TextInput**:
```rust
pub struct TextInput {
    value: Signal<String>,
    placeholder: String,
    on_submit: Option<Box<dyn Fn(&str)>>,
}

impl Widget for TextInput {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext);
    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult;
}
```

### 5.4 Main Application (`coder_app`)

**Goal**: Entry point that bootstraps the full application.

**Files to create**:
- `crates/coder_app/Cargo.toml`
- `crates/coder_app/src/main.rs` - Native entry
- `crates/coder_app/src/lib.rs` - WASM entry

**Main**:
```rust
fn main() {
    // Initialize platform (web or desktop)
    let platform = Platform::init();

    // Create application state
    let app = App::new();

    // Run event loop
    platform.run(move |event| {
        app.handle_event(event);
        app.run_frame();
    });
}
```

---

## Phase 6: Additional IDE Surfaces

### 6.1 Terminal Surface

**Goal**: ANSI-capable terminal emulator rendered via wgpui.

**Files**:
- `crates/coder_surfaces_terminal/Cargo.toml`
- `crates/coder_surfaces_terminal/src/lib.rs`
- `crates/coder_surfaces_terminal/src/ansi.rs` - ANSI parsing (vte crate)
- `crates/coder_surfaces_terminal/src/buffer.rs` - Scrollback buffer
- `crates/coder_surfaces_terminal/src/terminal.rs` - Terminal widget

### 6.2 Diff Viewer Surface

**Goal**: Side-by-side and inline diff rendering.

**Files**:
- `crates/coder_surfaces_diff/Cargo.toml`
- `crates/coder_surfaces_diff/src/lib.rs`
- `crates/coder_surfaces_diff/src/diff.rs` - Diff computation
- `crates/coder_surfaces_diff/src/view.rs` - DiffViewer widget

### 6.3 Run Timeline Surface

**Goal**: Visualize agent workflow execution.

**Files**:
- `crates/coder_surfaces_timeline/Cargo.toml`
- `crates/coder_surfaces_timeline/src/lib.rs`
- `crates/coder_surfaces_timeline/src/timeline.rs` - Timeline widget

---

## Phase 7: Production Polish

### 7.1 Accessibility

- Semantics tree generation from widget tree
- Web: ARIA attributes via hidden DOM mirror
- Desktop: Platform a11y APIs (AX/UIA/AT-SPI)

### 7.2 IME Integration

- Web: Hidden textarea for CJK composition
- Desktop: Native IME hooks via winit
- Composition preview rendering

### 7.3 Mobile Platforms

- iOS: Metal surface
- Android: Vulkan surface

---

## First Milestone: Chat Surface End-to-End (No Dioxus)

**Goal**: Fully functional chat interface using our stack, running on both web and desktop.

### Deliverables

1. **Application shell** (Phase 5.1)
   - Basic router with Chat route
   - Minimal chrome (just a container)

2. **Chat surface** (Phase 5.2)
   - ChatThread widget with virtual scrolling
   - MessageBubble with markdown rendering
   - Tool use indicators

3. **Input widgets** (Phase 5.3)
   - TextInput for composing messages
   - Submit button

4. **Main application** (Phase 5.4)
   - Web and desktop entry points
   - Event loop integration

### Success Criteria

- [ ] 60fps scrolling with 500+ messages
- [ ] Streaming text renders smoothly (no flicker)
- [ ] Works identically on web and desktop
- [ ] Markdown rendering matches current wgpui demo
- [ ] Selection and copy-to-clipboard work
- [ ] Text input works with IME (basic)
- [ ] **No Dioxus code used anywhere**

---

## Summary: What's Done vs. What's Next

### ✅ Complete (Phases 1-4)

| Phase | Crates | Tests |
|-------|--------|-------|
| Phase 1 | wgpui (input, hit_test, scroll, platform) | - |
| Phase 2 | coder_domain, coder_protocol | 15 |
| Phase 3 | coder_ui_runtime | 20 |
| Phase 4 | coder_widgets | 8 |

**Total: 43 tests passing**

### 🔄 In Progress (Phase 5)

| Crate | Status |
|-------|--------|
| coder_shell | Not started |
| coder_surfaces_chat | Not started |
| coder_app | Not started |

### 📋 Planned (Phases 6-7)

| Crate | Status |
|-------|--------|
| coder_surfaces_terminal | Not started |
| coder_surfaces_diff | Not started |
| coder_surfaces_timeline | Not started |

---

## Migration Path: Removing Dioxus

Once the new stack is functional:

1. **Parallel operation**: New coder_app can run alongside old dioxus crate during transition
2. **Feature parity**: Implement all MechaCoder features in coder_surfaces_chat
3. **Cutover**: Remove dioxus crate from workspace
4. **Cleanup**: Remove dioxus-related dependencies

The key is that **both stacks can coexist** during migration - they're separate binaries.
