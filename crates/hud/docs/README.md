# HUD - Sci-Fi UI Components for WGPUI

HUD is a GPU-accelerated sci-fi UI component library inspired by [Arwes](https://arwes.dev/), adapted for the WGPUI rendering system. It provides futuristic, animated UI components with a white-on-black color scheme.

## Features

- **GPU-Accelerated Rendering**: All components render through WGPUI's efficient GPU pipeline
- **Arwes-Inspired Animations**: Smooth enter/exit animations with configurable easing
- **Staggered Orchestration**: AnimatorManager coordinates child animations with parallel, stagger, or sequence modes
- **Theme System**: Consistent white/opacity color scheme with predefined constants
- **30+ Easing Functions**: Full suite of easing curves (quad, cubic, expo, elastic, bounce, etc.)

## Quick Start

```rust
use hud::{HudAnimator, FrameCorners, HudButton, colors};
use wgpui::{Scene, Bounds};

// Create a button
let mut button = HudButton::new("CONNECT")
    .font_size(14.0)
    .corner_length(12.0);

// Start the enter animation
button.animator_mut().enter();

// In your update loop:
button.tick();

// In your paint function:
button.paint(bounds, &mut scene, &mut text_system);
```

## Documentation

- [Getting Started](getting-started.md) - Setup and basic usage
- [Animation System](animator.md) - HudAnimator and AnimatorManager
- [Components Reference](components.md) - All available components
- [Theming](theming.md) - Color constants and customization
- [Easing Functions](easing.md) - Available easing curves

## Component Categories

### Frames
Border and container components for structuring layouts.

| Component | Description |
|-----------|-------------|
| `FrameCorners` | Bracket-style corner decorations |
| `FrameLines` | Edge lines with configurable gaps |
| `FrameOctagon` | 8-sided frame with clipped corners |
| `FrameCircle` | Circular border using segments |
| `FrameHeader` | Header section with top line and accents |
| `FrameUnderline` | Simple animated bottom line |

### Backgrounds
Animated background patterns for atmosphere.

| Component | Description |
|-----------|-------------|
| `DotGridBackground` | Animated dot grid pattern |
| `GridLinesBackground` | Static grid line pattern |
| `MovingLinesBackground` | Animated moving lines |

### Text Animation
Dynamic text effects.

| Component | Description |
|-----------|-------------|
| `TextSequence` | Character-by-character typewriter reveal |
| `TextDecipher` | Scramble/decipher "hacking" effect |

### Effects
Visual enhancement effects.

| Component | Description |
|-----------|-------------|
| `Illuminator` | Mouse-following radial glow |

### Interactive
User interaction components.

| Component | Description |
|-----------|-------------|
| `HudButton` | Animated button with frame and hover states |

### Form Controls
Input components for user data entry.

| Component | Description |
|-----------|-------------|
| `TextInput` | Text field with animated underline |
| `Checkbox` | Animated checkmark with label |
| `Toggle` | On/off switch with sliding knob |
| `Select` | Dropdown menu with animated expansion |

### Data Display
Components for presenting structured data.

| Component | Description |
|-----------|-------------|
| `List` | Animated list with bullet markers |
| `Table` | Data table with headers and rows |
| `CodeBlock` | Code display with line numbers |
| `Card` | Content container with title |

## Architecture

```
hud/
  animator/      - Animation state machine and orchestration
  background/    - Background pattern components
  button/        - Interactive button
  data/          - Data display components
  effects/       - Visual effects
  form/          - Form input components
  frame/         - Frame and border components
  text/          - Text animation components
  easing.rs      - Easing function library
  theme.rs       - Color and timing constants
```

## Running the Demo

```bash
cargo run -p hud --bin hud_demo
```

The demo showcases all available components with their animations.
