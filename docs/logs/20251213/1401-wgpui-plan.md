# Plan: Bootstrap wgpui + Chat UI Surface

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     coder_ui (owned)                        │
│  - Command bus, app state, view model, theming tokens       │
├─────────────────────────────────────────────────────────────┤
│  ui_shell (Dioxus adapter)  │  ui_surface (wgpui adapter)   │
│  - Auth, settings, forms    │  - Chat thread, terminal      │
│  - Project list, routing    │  - Diff viewer, timeline      │
│  - HTML input (IME)         │  - GPU-rendered canvases      │
└─────────────────────────────────────────────────────────────┘
```

**Key principle**: Own the pixels for core surfaces, borrow Dioxus for shell/forms/input.

---

## Phase 1: wgpui Foundation (Files to Create)

### 1.1 Crate Setup

Create `crates/wgpui/` with source code:

```
crates/wgpui/
├── Cargo.toml
├── Trunk.toml
├── index.html
└── src/
    ├── lib.rs                 # Public API
    ├── color.rs               # Hsla color type
    ├── geometry.rs            # Point, Size, Bounds
    ├── element.rs             # Element trait, AnyElement
    ├── styled.rs              # Styled trait (Tailwind-like API)
    ├── layout.rs              # Taffy 0.9 integration
    ├── text.rs                # TextSystem + glyph atlas (cosmic-text)
    ├── scene.rs               # Quad, TextRun primitives
    ├── theme.rs               # Bloomberg-dark theme colors
    ├── renderer.rs            # WgpuRenderer
    ├── shaders/
    │   ├── quad.wgsl          # SDF rounded corners/borders
    │   └── text.wgsl          # Glyph atlas sampling
    └── platform/
        └── web.rs             # WebPlatform (canvas init, event loop)
```

### 1.2 Core Dependencies

```toml
[dependencies]
wgpu = { version = "24.0", features = ["webgpu", "webgl"] }
taffy = "0.9"
cosmic-text = "0.12"
bytemuck = { version = "1", features = ["derive"] }
smallvec = "1"
slotmap = "1"

# Web
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = "0.3"
js-sys = "0.3"
getrandom = { version = "0.3", features = ["wasm_js"] }
```

### 1.3 Core Primitives

**Element trait** (two-phase rendering):
```rust
pub trait Element: 'static {
    type State: Default;
    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::State);
    fn paint(&mut self, bounds: Bounds, state: &mut Self::State, cx: &mut PaintContext);
}
```

**Styled trait** (fluent API):
```rust
pub trait Styled: Sized {
    fn style(&mut self) -> &mut Style;
    fn flex(self) -> Self;
    fn bg(self, color: impl Into<Hsla>) -> Self;
    fn p(self, padding: impl Into<Length>) -> Self;
    // ... 30+ methods
}
```

**Scene primitives**:
```rust
pub struct Quad { bounds, background, border, corner_radii }
pub struct TextRun { glyphs, origin, color }
```

---

## Phase 2: Chat Surface Components

### 2.1 Chat Module

Create `crates/wgpui/src/chat/`:

```
src/chat/
├── mod.rs                    # Chat exports
├── message_thread.rs         # Scrollable message list
├── message_bubble.rs         # User vs assistant styling
├── tool_status.rs            # Tool execution indicator
└── markdown.rs               # Markdown to styled spans
```

### 2.2 Key Components

**MessageThread**: Virtual scrolling container for messages
- Calculates visible range based on scroll offset
- Only renders visible items (perf for 1000+ messages)
- Handles wheel events for scrolling

**MessageBubble**: Renders single message
- User: ">" prefix in #FFB400, plain text
- Assistant: Markdown rendered as styled spans

**ToolStatus**: Tool execution line
- Status indicator (green/red/yellow)
- Tool name + truncated input preview

**Markdown**: Parse markdown to styled text spans
- Headers, bold, italic, code, lists
- Defer code block syntax highlighting to later

---

## Phase 3: Dioxus Integration

### 3.1 WgpuiCanvas Component

Create `crates/dioxus/src/components/wgpui_canvas.rs`:

```rust
#[component]
pub fn WgpuiCanvas(
    id: String,
    entries: Signal<Vec<ThreadEntry>>,
    streaming_text: Signal<String>,
) -> Element {
    // Initialize wgpui on canvas mount
    // Bridge Dioxus signals → wgpui state updates
    rsx! {
        canvas { id: "{id}", style: "width: 100%; height: 100%;" }
    }
}
```

### 3.2 Hybrid MechaCoder

Update `crates/dioxus/src/views/mechacoder.rs`:

```rust
rsx! {
    div { style: "display: flex; height: 100vh;",
        // Sidebar (keep Dioxus/SVG)
        ConversationGraph { entries, on_node_click }

        div { style: "flex: 1; display: flex; flex-direction: column;",
            // Message thread - wgpui canvas (GPU-rendered)
            div { style: "flex: 1;",
                WgpuiCanvas { id: "chat", entries, streaming_text }
            }

            // Input area - keep HTML (IME support)
            div { style: "border-top: 1px solid #1A1A1A;",
                input { /* existing implementation */ }
            }
        }
    }
}
```

---

## Phase 4: Polish & Integration

- Smooth scrolling with momentum
- Scroll-to-entry from sidebar clicks
- Loading/streaming text animation
- Error states and fallbacks

---

## Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` (workspace) | Add `wgpui` to members |
| `crates/dioxus/Cargo.toml` | Add `wgpui` dependency |
| `crates/dioxus/src/views/mechacoder.rs` | Add WgpuiCanvas integration |

## Files to Create

| File | Purpose |
|------|---------|
| `crates/wgpui/Cargo.toml` | Crate config |
| `crates/wgpui/src/lib.rs` | Public API |
| `crates/wgpui/src/element.rs` | Element trait |
| `crates/wgpui/src/styled.rs` | Styled trait |
| `crates/wgpui/src/layout.rs` | Taffy integration |
| `crates/wgpui/src/text.rs` | cosmic-text + atlas |
| `crates/wgpui/src/renderer.rs` | wgpu pipelines |
| `crates/wgpui/src/platform/web.rs` | WebPlatform |
| `crates/wgpui/src/chat/*.rs` | Chat components |
| `crates/dioxus/src/components/wgpui_canvas.rs` | Dioxus bridge |

---

## Implementation Order

### Week 1: wgpui Core
1. Create crate structure with Cargo.toml, Trunk.toml
2. Implement `Hsla`, `Point`, `Size`, `Bounds`
3. Implement `LayoutEngine` (Taffy wrapper)
4. Implement `TextSystem` (cosmic-text + glyph atlas)
5. Implement `WgpuRenderer` (quad + text pipelines)
6. Implement `WebPlatform` (canvas init)
7. Demo: Render static quads + text

### Week 2: Element System
1. Implement `Element` trait + `AnyElement`
2. Implement `Styled` trait
3. Implement `Div` element with children
4. Implement `Text` element
5. Implement `ScrollView` with clipping

### Week 3: Chat Components
1. Implement `MessageThread` (virtual scrolling)
2. Implement `MessageBubble` (user/assistant)
3. Implement `ToolStatus`
4. Implement `MarkdownRenderer` (basic)

### Week 4: Integration
1. Create `WgpuiCanvas` Dioxus component
2. Signal → wgpui state bridge
3. Update MechaCoder to hybrid architecture
4. Test with existing WebSocket backend

---

## Success Criteria

- [ ] 60fps with 500+ messages (virtual scrolling)
- [ ] Pixel-perfect Bloomberg-dark theme
- [ ] Works in Chrome, Firefox, Safari
- [ ] Seamless state sync between Dioxus signals and wgpui
- [ ] HTML input preserved for IME support
