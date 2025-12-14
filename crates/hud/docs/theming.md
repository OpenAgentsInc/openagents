# Theming

HUD uses a white-on-black color scheme with varying opacities, inspired by the Arwes framework.

## Color Constants

Import colors from the theme module:

```rust
use hud::colors;
// or
use hud::theme::hud as colors;
```

### Frame Colors

For borders, lines, and structural elements:

| Constant | HSLA | Description |
|----------|------|-------------|
| `FRAME_BRIGHT` | `hsla(0, 0%, 100%, 0.8)` | High visibility, active states |
| `FRAME_NORMAL` | `hsla(0, 0%, 100%, 0.5)` | Default frame color |
| `FRAME_DIM` | `hsla(0, 0%, 100%, 0.2)` | Subtle, inactive states |

### Text Colors

For typography:

| Constant | HSLA | Description |
|----------|------|-------------|
| `TEXT` | `hsla(0, 0%, 100%, 0.9)` | Primary text |
| `TEXT_MUTED` | `hsla(0, 0%, 100%, 0.5)` | Secondary, less important text |

### Background Colors

| Constant | HSLA | Description |
|----------|------|-------------|
| `BG` | `hsla(0, 0%, 0%, 1.0)` | Pure black background |
| `DOT_GRID` | `hsla(0, 0%, 100%, 0.06)` | Dot grid opacity |

## Timing Constants

Animation timing values (in frames at 60fps):

```rust
use hud::theme::timing;
```

| Constant | Value | Description |
|----------|-------|-------------|
| `ENTER_FRAMES` | `20` | Default enter animation (~0.33s) |
| `EXIT_FRAMES` | `15` | Default exit animation (~0.25s) |
| `STAGGER_OFFSET` | `3` | Frames between staggered items |

## Using Colors

### With Components

Most components accept color through a fluent method:

```rust
use hud::{FrameCorners, colors};

let frame = FrameCorners::new()
    .color(colors::FRAME_BRIGHT);
```

### With Opacity Modification

Adjust opacity based on animation progress:

```rust
use wgpui::Hsla;
use hud::colors;

fn paint(&self, scene: &mut Scene) {
    let progress = self.animator.progress();

    // Fade in with animation
    let color = Hsla::new(
        colors::FRAME_NORMAL.h,
        colors::FRAME_NORMAL.s,
        colors::FRAME_NORMAL.l,
        colors::FRAME_NORMAL.a * progress,
    );

    // ... use color
}
```

### Creating Custom Colors

```rust
use wgpui::Hsla;

// HSLA: hue, saturation, lightness, alpha
let custom_cyan = Hsla::new(0.5, 0.8, 0.6, 0.7);

// Pure white with custom opacity
let subtle_white = Hsla::new(0.0, 0.0, 1.0, 0.1);
```

## Creating a Custom Theme

You can create your own theme module:

```rust
// my_theme.rs
use wgpui::Hsla;

pub mod colors {
    use super::Hsla;

    // Cyan accent theme
    pub const FRAME_BRIGHT: Hsla = Hsla::new(0.5, 0.8, 0.6, 0.9);
    pub const FRAME_NORMAL: Hsla = Hsla::new(0.5, 0.7, 0.5, 0.6);
    pub const FRAME_DIM: Hsla = Hsla::new(0.5, 0.6, 0.4, 0.3);

    pub const TEXT: Hsla = Hsla::new(0.5, 0.3, 0.9, 0.95);
    pub const TEXT_MUTED: Hsla = Hsla::new(0.5, 0.2, 0.7, 0.6);

    pub const BG: Hsla = Hsla::new(0.55, 0.3, 0.05, 1.0);
}

pub mod timing {
    pub const ENTER_FRAMES: u32 = 25;
    pub const EXIT_FRAMES: u32 = 20;
    pub const STAGGER_OFFSET: u32 = 4;
}
```

Then use it with components:

```rust
use my_theme::colors as theme;

let frame = FrameCorners::new()
    .color(theme::FRAME_BRIGHT);
```

## State-Based Coloring

Common pattern for hover/active states:

```rust
fn get_frame_color(&self) -> Hsla {
    if self.is_pressed {
        colors::FRAME_DIM
    } else if self.is_hovered {
        colors::FRAME_BRIGHT
    } else {
        colors::FRAME_NORMAL
    }
}

fn get_text_color(&self) -> Hsla {
    if self.is_active {
        colors::TEXT
    } else {
        colors::TEXT_MUTED
    }
}
```

## Background Patterns

Recommended opacity levels for layered backgrounds:

```rust
// Layer 1: Grid lines (subtle structure)
let grid = GridLinesBackground::new()
    .color(Hsla::new(0.0, 0.0, 1.0, 0.03));

// Layer 2: Moving lines (atmosphere)
let lines = MovingLinesBackground::new()
    .color(Hsla::new(0.0, 0.0, 1.0, 0.02));

// Layer 3: Dot grid (texture)
let dots = DotGridBackground::new()
    .color(Hsla::new(0.0, 0.0, 1.0, 0.06));

// Paint back to front
grid.paint(bounds, &mut scene);
lines.paint(bounds, &mut scene);
dots.paint(bounds, &mut scene);
```

## Accessibility Considerations

The default theme uses relatively high contrast (white on black). When customizing:

1. **Maintain contrast ratio** - Primary text should have at least 4.5:1 contrast
2. **Use opacity for hierarchy** - Brighter = more important
3. **Consider color blindness** - Don't rely solely on hue for meaning
4. **Animation duration** - Keep animations under 0.5s for most elements

## Integration with WGPUI

HUD colors are WGPUI `Hsla` values, compatible with all WGPUI primitives:

```rust
use wgpui::{Quad, Bounds};
use hud::colors;

scene.draw_quad(
    Quad::new(bounds)
        .with_background(colors::BG)
        .with_border(colors::FRAME_NORMAL, 1.0)
);
```
