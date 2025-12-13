# wgpui

GPU-accelerated UI rendering library for Rust. Own the pixel.

## Overview

wgpui is a cross-platform GPU-accelerated rendering library designed for building high-performance UI surfaces. It provides:

- **GPU Rendering**: Hardware-accelerated via wgpu (WebGPU, WebGL, Vulkan, Metal, DX12)
- **Text Rendering**: High-quality text with cosmic-text shaping and glyph atlas
- **SDF Primitives**: Smooth rounded corners and borders using signed distance fields
- **Layout Engine**: CSS Flexbox via Taffy
- **Theme System**: Bloomberg-inspired dark theme

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Application                         │
├─────────────────────────────────────────────────────┤
│  Scene         │  Layout        │  Text             │
│  - Quad        │  - Taffy       │  - cosmic-text    │
│  - TextRun     │  - Flexbox     │  - Glyph atlas    │
├─────────────────────────────────────────────────────┤
│                  Renderer                            │
│  - Quad pipeline (SDF)                              │
│  - Text pipeline (atlas sampling)                   │
├─────────────────────────────────────────────────────┤
│                  Platform                            │
│  - Web (wasm32 + web-sys)                           │
│  - Native (winit) [future]                          │
└─────────────────────────────────────────────────────┘
```

## Quick Start

```rust
use wgpui::{Scene, Quad, Bounds, theme};

// Create a scene
let mut scene = Scene::new();

// Draw a rounded rectangle
scene.draw_quad(
    Quad::new(Bounds::new(10.0, 10.0, 200.0, 100.0))
        .with_background(theme::bg::SURFACE)
        .with_border(theme::border::DEFAULT, 1.0)
        .with_uniform_radius(8.0)
);

// Draw text
let text_run = text_system.layout(
    "Hello, wgpui!",
    Point::new(20.0, 30.0),
    14.0,
    theme::text::PRIMARY
);
scene.draw_text(text_run);
```

## Running the Demo

```bash
cd crates/wgpui
trunk serve --open
```

This starts a development server at `http://127.0.0.1:8081` with hot reload.

## Documentation

- [Architecture](docs/ARCHITECTURE.md) - System design and module structure
- [Rendering](docs/RENDERING.md) - GPU rendering pipeline and shaders
- [Text System](docs/TEXT.md) - Text shaping, glyph atlas, and rendering
- [Layout Engine](docs/LAYOUT.md) - Taffy integration and CSS Flexbox
- [Theme System](docs/THEME.md) - Colors, typography, and design tokens
- [Integration](docs/INTEGRATION.md) - Embedding in Dioxus and other frameworks

## Features

### Quad Rendering (SDF)

- Per-corner radius support
- Configurable border width and color
- Anti-aliased edges via signed distance fields
- Premultiplied alpha blending

### Text Rendering

- Full Unicode support via cosmic-text
- Advanced text shaping (ligatures, kerning)
- Efficient glyph atlas with on-demand rasterization
- High-DPI support with scale factor handling

### Layout Engine

- CSS Flexbox via Taffy 0.9
- Flex direction, wrap, gap
- Justify content, align items
- Min/max sizing constraints
- Padding and margin

### Theme System

Bloomberg Terminal-inspired dark theme:

```rust
use wgpui::theme;

theme::bg::APP          // #000000 - Pure black
theme::bg::SURFACE      // #0A0A0A - Near black
theme::text::PRIMARY    // #E6E6E6 - Main text
theme::accent::PRIMARY  // #FFB400 - Bloomberg yellow
theme::status::SUCCESS  // #00C853 - Green
theme::status::ERROR    // #D32F2F - Red
```

## Platform Support

| Platform | Status | Backend |
|----------|--------|---------|
| Web (Chrome) | ✅ | WebGPU |
| Web (Firefox) | ✅ | WebGPU/WebGL |
| Web (Safari) | ✅ | WebGPU |
| macOS | 🚧 | Metal |
| Linux | 🚧 | Vulkan |
| Windows | 🚧 | DX12/Vulkan |

## Dependencies

- `wgpu 24.0` - GPU abstraction
- `taffy 0.9` - CSS Flexbox layout
- `cosmic-text 0.12` - Text shaping and rasterization
- `bytemuck 1` - GPU struct marshaling

## License

MIT
