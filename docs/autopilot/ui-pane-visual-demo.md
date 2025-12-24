# UI Pane Visual Demo

A standalone GPU-accelerated visual demo showing agent-controlled UI panes using the wgpui Frame component.

## Overview

This demo visualizes how an AI agent manipulates UI panes through tool calls. It uses:
- **wgpui** - GPU-accelerated rendering via wgpu
- **Arwes-style Frames** - Sci-fi UI frames (Corners, Lines, Octagon, Nefrex, Kranox)
- **Animation system** - Smooth transitions for movement, resize, glow, and state changes
- **Tool call simulation** - Demonstrates the `ui_pane` tool in action

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    UI Pane Visual Demo                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    Window (winit)                        │   │
│  │  ┌─────────────────────────────────────────────────┐    │   │
│  │  │              GPU Surface (wgpu)                  │    │   │
│  │  │                                                  │    │   │
│  │  │   ┌─────────────┐  ┌─────────────┐              │    │   │
│  │  │   │ Editor Pane │  │  Chat Pane  │              │    │   │
│  │  │   │ (Frame)     │  │  (Frame)    │              │    │   │
│  │  │   └─────────────┘  └─────────────┘              │    │   │
│  │  │                                                  │    │   │
│  │  │   ┌─────────────┐  ┌─────────────┐              │    │   │
│  │  │   │Terminal Pane│  │Diagnostics  │              │    │   │
│  │  │   │ (Frame)     │  │  (Frame)    │              │    │   │
│  │  │   └─────────────┘  └─────────────┘              │    │   │
│  │  │                                                  │    │   │
│  │  └──────────────────────────────────────────────────┘    │   │
│  │                                                          │   │
│  │  ┌──────────────────────────────────────────────────┐    │   │
│  │  │              Tool Call Log (bottom)               │    │   │
│  │  │  > CreatePane "editor" at (50, 50)               │    │   │
│  │  │  > SetPriority "diagnostics" Urgent              │    │   │
│  │  │  > Animate "diagnostics" Pulse                   │    │   │
│  │  └──────────────────────────────────────────────────┘    │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Pane State

Each visual pane tracks:
- **Position** - x, y coordinates (animated)
- **Size** - width, height (animated)
- **Priority** - affects glow color and z-index
- **Frame style** - Corners, Lines, Octagon, Underline, Nefrex, Kranox
- **Glow color** - Optional attention indicator
- **Animation progress** - 0.0 to 1.0 for entry/exit animations
- **State** - Open, Minimized, SlideIn, SlideOut, etc.

## Demo Scenarios

The demo runs through a scripted sequence:

### 1. Workspace Setup (0-2s)
- Create Editor pane (slide in from left)
- Create Terminal pane (slide in from bottom)
- Create Chat pane (slide in from right)
- Create Diagnostics pane (fade in)

### 2. Error Found (2-4s)
- Diagnostics pane: priority → Urgent (orange glow)
- Diagnostics pane: bring to front (z-index bump)
- Diagnostics pane: Pulse animation (3x)
- Diagnostics pane: frame style → Kranox

### 3. Showing Fix (4-6s)
- Diagnostics: priority → Normal (glow off)
- Editor pane: focus + green glow
- Editor pane: slight size increase

### 4. Running Tests (6-8s)
- Terminal pane: animated move (y: 670 → 400)
- Terminal pane: animated resize (height: 200 → 400)
- Terminal pane: priority → Elevated (blue glow)

### 5. Success (8-10s)
- Terminal: minimize (shrink animation)
- Chat: attention request (red glow + shake)
- Show "All tests passed!" message

### 6. Cleanup (10-12s)
- Diagnostics: slide out
- Reset to initial state
- Loop

## Tool Call Log

The bottom of the screen shows a scrolling log of tool calls:
```
[0.5s] ui_pane.CreatePane { id: "editor", title: "Code Editor" }
[0.8s] ui_pane.CreatePane { id: "terminal", title: "Terminal" }
[1.1s] ui_pane.CreatePane { id: "chat", title: "AI Assistant" }
[1.4s] ui_pane.CreatePane { id: "diagnostics", title: "Diagnostics" }
[2.0s] ui_pane.SetPriority { id: "diagnostics", priority: "Urgent" }
[2.2s] ui_pane.Focus { id: "diagnostics" }
[2.4s] ui_pane.Animate { id: "diagnostics", animation: "Pulse" }
...
```

## Key Bindings

| Key | Action |
|-----|--------|
| `Space` | Pause/resume demo |
| `R` | Restart demo from beginning |
| `1-6` | Jump to scenario 1-6 |
| `Escape` | Exit |

## Running

```bash
cargo run -p wgpui --example ui_pane_demo --features desktop
```

## Implementation Details

### Pane Animation

Each pane uses `Animation<f32>` for:
- Position X/Y transitions
- Size W/H transitions
- Opacity/alpha transitions

And `SpringAnimation` for:
- Bounce effects on focus
- Shake effects on attention

### Frame Rendering

Each pane renders using `Frame::*()` from wgpui:
```rust
Frame::corners()
    .line_color(pane.line_color())
    .bg_color(pane.bg_color())
    .glow_color(pane.glow_color())
    .stroke_width(2.0)
    .animation_progress(pane.animation_progress)
    .paint(pane.current_bounds(), &mut cx);
```

### Priority to Glow Mapping

| Priority | Glow Color |
|----------|------------|
| Background | None |
| Normal | None |
| Elevated | Cyan (#00a8ff) |
| Urgent | Orange (#ff6600) |
| Critical | Red (#ff0000) |

### Frame Style to Type Mapping

| Priority | Frame Style |
|----------|-------------|
| Background | Lines |
| Normal | Corners |
| Elevated | Corners |
| Urgent | Kranox |
| Critical | Kranox |

## Dependencies

- `wgpui` - rendering, Frame, Animation, theme
- `winit` - windowing
- `wgpu` - GPU backend
- `pollster` - async block for desktop

## Files

- `crates/wgpui/examples/ui_pane_demo.rs` - Main demo binary
- `docs/autopilot/ui-pane-visual-demo.md` - This documentation
