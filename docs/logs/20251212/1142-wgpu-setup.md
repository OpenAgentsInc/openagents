# WGPUI Framework Setup - 2024-12-12 11:42

## Overview

Created `wgpui`, a cross-platform wgpu-based UI framework for running MechaCoder on both web (WASM) and native platforms. This is Milestone 1 of the WGPUI Foundation plan.

## Goals

- Build a GPUI-inspired element system that works on web
- Use wgpu for cross-platform GPU rendering
- Use Taffy for CSS Flexbox layout
- Use cosmic-text for text rendering
- Web-first development approach

## Files Created

### Crate Structure
```
crates/wgpui/
├── Cargo.toml          # Web-first features, wgpu + taffy + cosmic-text
├── Trunk.toml          # WASM build config
├── index.html          # Demo entry point
└── src/
    ├── lib.rs          # Public API + demo entry point
    ├── color.rs        # Hsla color type
    ├── element.rs      # Element trait, AnyElement, ParentElement
    ├── elements/
    │   ├── mod.rs
    │   ├── div.rs      # Div container element
    │   └── text_element.rs  # Text element
    ├── layout.rs       # Taffy layout engine integration
    ├── platform/
    │   ├── mod.rs      # Event types (Resize, Mouse, Key, etc.)
    │   └── web.rs      # WebPlatform (wasm32 only)
    ├── scene.rs        # Quad, TextQuad, Scene primitives
    ├── styled.rs       # Styled trait, Style struct
    ├── text.rs         # TextSystem with cosmic-text + glyph atlas
    └── theme.rs        # Bloomberg-style dark theme colors
```

### Files Modified
- `Cargo.toml` (workspace) - Added wgpui to members and dependencies

## Technical Details

### Element System

The Element trait provides a two-phase lifecycle:
1. `request_layout` - Request space from the Taffy layout engine
2. `paint` - Draw to the Scene after layout is computed

```rust
pub trait Element: 'static {
    type State: 'static + Default;
    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::State);
    fn paint(&mut self, bounds: Bounds, state: &mut Self::State, cx: &mut PaintContext);
}
```

### Styled Trait

Tailwind-like fluent API for styling:
```rust
div()
    .flex()
    .flex_col()
    .w_full()
    .h_full()
    .bg(theme::bg::APP)
    .p(20.0)
    .gap(16.0)
    .child(text("Hello!").color(theme::text::PRIMARY))
```

### Layout Engine

Integrated Taffy for CSS Flexbox layout:
- `LayoutEngine` wraps `TaffyTree`
- `LayoutId` for tracking nodes
- Style conversion from our Style struct to Taffy styles
- Support for flex, sizing, padding, margin, gap, position

### Text Rendering

Ported from openagents-web with cosmic-text:
- `TextSystem` manages font loading (Berkeley Mono)
- Glyph atlas for efficient GPU rendering
- High-DPI support via scale factor
- Text measurement for layout

### Web Platform

WebPlatform for browser rendering:
- wgpu with WebGPU/WebGL backends
- Canvas-based surface creation
- Quad shader with rounded corners and borders
- Text shader with glyph atlas sampling
- Animation loop via requestAnimationFrame
- Resize handling

### Theme

Bloomberg Terminal inspired dark theme:
- `theme::bg::APP`, `theme::bg::SURFACE`, `theme::bg::CARD`
- `theme::text::PRIMARY`, `theme::text::SECONDARY`, `theme::text::MUTED`
- `theme::accent::PRIMARY` (yellow), `theme::accent::BLUE`, etc.
- `theme::border::DEFAULT`, `theme::border::FOCUS`

## Issues Encountered & Solutions

### 1. Taffy API Changes
Taffy 0.9 uses lowercase constructor functions instead of enum variants:
- `taffy::Dimension::Length(x)` → `taffy::Dimension::length(x)`
- `taffy::Dimension::Auto` → `taffy::Dimension::auto()`
- `taffy::LengthPercentage::Length(x)` → `taffy::LengthPercentage::length(x)`

### 2. WASM-only Surface Target
`wgpu::SurfaceTarget::Canvas` only exists for wasm32 target. Added cfg attributes:
```rust
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web;
```

## Build Status

- Native: `cargo check -p wgpui` - Success
- WASM: `cargo check -p wgpui --target wasm32-unknown-unknown` - Success
- Demo: `trunk serve` from crates/wgpui - Running on http://127.0.0.1:8081/

## Next Steps (Future Milestones)

### Milestone 2: Components & Styling
- Port Button, Input, Card components
- Event handling (click, keyboard, focus)
- Hover states

### Milestone 3: Authentication Backend
- Create `crates/api-server/` with Actix Web
- GitHub OAuth endpoints
- Token generation/validation

### Milestone 4: MechaCoder Web
- WebSocket connection to api-server for Claude API proxy
- Simplified web version of MechaCoder

### Milestone 5: AWS Deployment
- S3 + CloudFront for static WASM hosting
- ECS or Lambda for api-server

## References

- Plan file: `.claude/plans/wondrous-rolling-feather.md`
- Reference implementation: `crates/openagents-web/`
- GPUI patterns: `crates/gpui/`
