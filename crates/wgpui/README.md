# wgpui

**GPU-Accelerated UI Rendering Library**

A cross-platform GPU-accelerated UI rendering library built on wgpu (WebGPU/Vulkan/Metal/DX12). Designed for high-performance canvas rendering in OpenAgents' hybrid UI architecture.

## Status

| Phase | Description | Status |
|-------|-------------|--------|
| **Phase 1** | Core Framework | ✅ Complete |
| **Phase 2** | Component System | ✅ Complete |
| Phase 3 | Atoms | ⏳ Planned |
| Phase 4 | Molecules | ⏳ Planned |
| Phase 5 | Organisms | ⏳ Planned |
| Phase 6 | Markdown & Streaming | 🚧 In Progress |
| Phase 7 | Sections (Surfaces) | ⏳ Planned |
| Phase 8 | HUD Components | ⏳ Planned |
| Phase 9 | Integration | ⏳ Planned |
| Phase 10 | Testing & Docs | ⏳ Planned |

### Phase 1: Core Framework ✅

| Module | Description | Status |
|--------|-------------|--------|
| `color.rs` | HSLA color type with GPU-friendly derives | ✅ Complete |
| `geometry.rs` | Point, Size, Bounds, Edges primitives | ✅ Complete |
| `scene.rs` | Quad, TextRun, Scene accumulator | ✅ Complete |
| `renderer.rs` | wgpu pipelines and GPU buffer management | ✅ Complete |
| `text.rs` | cosmic-text integration and glyph atlas | ✅ Complete |
| `layout.rs` | Taffy-based CSS Flexbox layout engine | ✅ Complete |
| `platform.rs` | Web (WASM) and desktop (winit) abstraction | ✅ Complete |
| `theme.rs` | Color and spacing tokens aligned with Tailwind | ✅ Complete |
| `scroll.rs` | Virtual scrolling containers | ✅ Complete |
| `input.rs` | Platform-agnostic input events | ✅ Complete |
| `first_light` | Visual demo example | ✅ Complete |

### Phase 2: Component System ✅

| Component | Description | Status |
|-----------|-------------|--------|
| `Component` trait | Core component abstraction with paint/event | ✅ Complete |
| `Div` | Container component with background/border | ✅ Complete |
| `Text` | Text rendering with font styling | ✅ Complete |
| `Button` | Interactive button with variants | ✅ Complete |
| `TextInput` | Full keyboard/mouse input, cursor, focus | ✅ Complete |
| `ScrollView` | Scrollable container | ✅ Complete |
| `VirtualList` | Virtualized list for large datasets | ✅ Complete |
| `Modal` | Overlay dialog with backdrop | ✅ Complete |
| `Dropdown` | Select component with keyboard navigation | ✅ Complete |
| `Tabs` | Tab bar with active indicator | ✅ Complete |
| `AnyComponent` | Type-erased component wrapper | ✅ Complete |

**127 unit tests passing.**

## Why wgpui?

OpenAgents uses a **hybrid rendering model**:

| HTML Rendering (Maud/HTMX) | GPU Rendering (wgpui) |
|---------------------------|----------------------|
| Forms, settings, navigation | Chat threads |
| Dashboards | Terminal emulator |
| Good accessibility | Diff viewer |
| Easy styling | Timeline visualization |
| ~60fps max | 60+ fps, unlimited |
| DOM-limited scrolling | Virtual scrolling at scale |

**wgpui handles performance-critical surfaces** where HTML hits its limits:
- Streaming markdown at 100+ tokens/sec
- Scrolling through 10k+ messages without jank
- Frame-accurate animations
- Real-time syntax highlighting

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         wgpui Architecture                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                        Application                           │   │
│  │   Atoms → Molecules → Organisms → Sections                   │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                    Component System                          │   │
│  │  Component trait, Div, Text, Button, TextInput, VirtualList  │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│  ┌──────────────┬──────────────┬──────────────┬────────────────┐   │
│  │    Layout    │     Text     │   Markdown   │     Scroll     │   │
│  │   (Taffy)    │ (cosmic-text)│  (pulldown)  │  (virtual)     │   │
│  └──────────────┴──────────────┴──────────────┴────────────────┘   │
│                              │                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                        Scene                                 │   │
│  │   Quad, TextRun, GpuQuad, GlyphInstance                     │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                       Renderer                               │   │
│  │   wgpu pipelines, shaders, GPU buffers                      │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│  ┌──────────────────────────┬──────────────────────────────────┐   │
│  │     Platform: Web        │      Platform: Desktop           │   │
│  │   (wasm-bindgen, web-sys)│       (winit, pollster)         │   │
│  └──────────────────────────┴──────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Core Modules

### Primitives

| Module | Description |
|--------|-------------|
| `color` | HSLA color type with GPU-friendly Pod/Zeroable derives |
| `geometry` | Point, Size, Bounds, Edges for layout and hit testing |
| `scene` | Accumulated draw primitives (Quad, TextRun) |
| `hit_test` | Point-in-bounds testing for input handling |
| `input` | Platform-agnostic input events (mouse, keyboard) |

### Rendering

| Module | Description |
|--------|-------------|
| `renderer` | wgpu render pipelines and GPU buffer management |
| `text` | Text shaping (cosmic-text) and glyph atlas |
| `theme` | Color and spacing tokens aligned with Tailwind |

### Layout & Composition

| Module | Description |
|--------|-------------|
| `layout` | Taffy-based CSS Flexbox layout engine |
| `scroll` | Virtual scrolling containers |
| `markdown` | Streaming markdown parser and renderer |
| `platform` | Web (WASM) and desktop (winit) platform abstraction |

## Usage

### Building

```bash
# Native build (default: web feature)
cargo build -p wgpui

# Desktop build
cargo build -p wgpui --features desktop --no-default-features

# WASM build
cargo build -p wgpui --target wasm32-unknown-unknown
```

### Basic Example

```rust
use wgpui::{Hsla, Point, Size, Bounds, Scene, Quad};

// Create a scene
let mut scene = Scene::new();

// Add a colored quad
let bounds = Bounds::new(10.0, 10.0, 200.0, 100.0);
let color = Hsla::from_hex(0x1a1a1a); // Dark surface color

scene.push_quad(Quad {
    bounds,
    background: Some(color),
    border_color: None,
    border_width: 0.0,
});

// Add text
scene.push_text(TextRun {
    text: "Hello, wgpui!".to_string(),
    bounds: Bounds::new(20.0, 20.0, 180.0, 30.0),
    color: Hsla::from_hex(0xfafafa), // Light text
    font_size: 14.0,
    font_weight: 400,
});
```

### Layout Example

```rust
use wgpui::{LayoutEngine, LayoutStyle, px, pct, auto};

let mut engine = LayoutEngine::new();

// Create a flex container
let container = engine.new_node(
    LayoutStyle::default()
        .width(pct(100.0))
        .height(auto())
        .flex_direction(FlexDirection::Column)
        .padding(px(16.0)),
);

// Add children
let header = engine.new_node(
    LayoutStyle::default()
        .height(px(48.0))
        .flex_shrink(0.0),
);

let content = engine.new_node(
    LayoutStyle::default()
        .flex_grow(1.0)
        .overflow(Overflow::Scroll),
);

engine.set_children(container, &[header, content]);

// Compute layout
engine.compute_layout(container, Size::new(800.0, 600.0));

// Get computed bounds
let header_bounds = engine.layout(header).bounds();
```

### Theme Tokens

```rust
use wgpui::theme;

// Background colors
let app_bg = theme::bg::APP;           // #0a0a0a
let surface_bg = theme::bg::SURFACE;   // #1a1a1a
let muted_bg = theme::bg::MUTED;       // #262626

// Text colors
let primary_text = theme::text::PRIMARY;  // #fafafa
let muted_text = theme::text::MUTED;      // #a1a1a1

// Accent colors
let accent = theme::accent::PRIMARY;   // #00a8ff
```

## Design Constraints

### Sharp Corners Only

**wgpui enforces sharp corners throughout.** The codebase has a pre-commit hook that rejects any code containing border radius functionality.

```rust
// ✅ Correct - sharp corners
let quad = Quad {
    bounds,
    background: Some(color),
    border_color: Some(border),
    border_width: 1.0,
};

// ❌ Forbidden - no corner_radius field exists
// let quad = Quad {
//     corner_radius: 8.0,  // This doesn't exist
// };
```

This aligns with OpenAgents' visual design language (Bloomberg terminal aesthetic).

### Theme Alignment

wgpui tokens must match Tailwind tokens used in HTML components:

| Tailwind Class | wgpui Token | Hex Value |
|----------------|-------------|-----------|
| `bg-background` | `theme::bg::APP` | `#0a0a0a` |
| `bg-card` | `theme::bg::SURFACE` | `#1a1a1a` |
| `bg-muted` | `theme::bg::MUTED` | `#262626` |
| `text-foreground` | `theme::text::PRIMARY` | `#fafafa` |
| `text-muted-foreground` | `theme::text::MUTED` | `#a1a1a1` |
| `border-border` | `theme::border::DEFAULT` | `#404040` |

### GPU-Friendly Types

Core types derive `bytemuck::Pod` and `bytemuck::Zeroable` for efficient GPU buffer uploads:

```rust
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Hsla {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}
```

## Performance Targets

| Metric | Target |
|--------|--------|
| Frame rate | 60 fps sustained |
| Input latency | <16ms |
| Scroll smoothness | No jank at 10k items |
| Streaming render | Real-time at 100 tokens/sec |
| WASM bundle size | <2MB gzipped |

## Dependencies

### Core

| Crate | Version | Purpose |
|-------|---------|---------|
| `wgpu` | 24.0 | GPU rendering (WebGPU/Vulkan/Metal/DX12) |
| `cosmic-text` | 0.12 | Text shaping and font rendering |
| `taffy` | 0.9 | CSS Flexbox layout |
| `bytemuck` | 1.x | GPU buffer casting |
| `pulldown-cmark` | 0.12 | Markdown parsing |
| `syntect` | 5.2 | Syntax highlighting |

### Web Platform (default)

| Crate | Purpose |
|-------|---------|
| `wasm-bindgen` | Rust-JavaScript interop |
| `web-sys` | Web API bindings |
| `js-sys` | JavaScript primitives |

### Desktop Platform (optional)

| Crate | Purpose |
|-------|---------|
| `winit` | Window management |
| `pollster` | Async runtime for sync contexts |

## Module Structure

```
src/
├── lib.rs              # Public API exports
├── color.rs            # Hsla color type
├── geometry.rs         # Point, Size, Bounds, Edges
├── scene.rs            # Quad, TextRun, Scene
├── renderer.rs         # wgpu pipelines
├── text.rs             # cosmic-text integration
├── layout.rs           # Taffy wrapper
├── scroll.rs           # Virtual scrolling
├── hit_test.rs         # Hit testing
├── input.rs            # Input events
├── theme.rs            # Color tokens
├── markdown.rs         # Markdown rendering
└── platform.rs         # Platform abstraction
    ├── web.rs          # WASM platform
    └── desktop.rs      # winit platform
```

## Future: Component Library

Once the core framework is complete, wgpui will include a full component library matching ACP (Anthropic Component Protocol) components:

### Atoms (13 components)
`status_dot`, `tool_icon`, `mode_badge`, `model_badge`, `streaming_indicator`, `thinking_toggle`, `tool_status_badge`, `permission_button`, `feedback_button`, `keybinding_hint`, `entry_marker`, `checkpoint_badge`, `content_type_icon`

### Molecules (10 components)
`message_header`, `tool_header`, `terminal_header`, `diff_header`, `thinking_block`, `permission_bar`, `mode_selector`, `model_selector`, `entry_actions`, `checkpoint_restore`

### Organisms (9 components)
`user_message`, `assistant_message`, `tool_call_card`, `terminal_tool_call`, `diff_tool_call`, `search_tool_call`, `thread_entry`, `thread_controls`, `permission_dialog`

### Sections (4 components)
`thread_view`, `thread_header`, `thread_feedback`, `message_editor`

## Related

- **Directive**: [d-020 WGPUI Integration](../../.openagents/directives/d-020.md)
- **Archive**: `~/code/backroom/archive/openagents/wgpui/` (reference implementations)
- **HTML UI**: `crates/ui/` (Maud/HTMX components)
- **ACP Reference**: `crates/ui/src/acp/` (target parity)

## License

Apache 2.0
