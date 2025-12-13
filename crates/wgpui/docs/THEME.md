# Theme System

## Overview

wgpui includes a Bloomberg Terminal-inspired dark theme with semantic color tokens, typography scales, and spacing values.

## Color Tokens

### Background Colors

```rust
use wgpui::theme::bg;

bg::APP       // #000000 - Pure black, main background
bg::SURFACE   // #0A0A0A - Near black, panels
bg::CODE      // #101010 - Code blocks
bg::CARD      // #0D0D0D - Elevated surfaces
bg::HOVER     // #141414 - Hover states
bg::SELECTED  // #1F1F1F - Selected items
```

### Text Colors

```rust
use wgpui::theme::text;

text::PRIMARY    // #E6E6E6 - Main text
text::SECONDARY  // #B0B0B0 - Less emphasis
text::MUTED      // #9E9E9E - Labels, hints
text::DISABLED   // #666666 - Disabled text
```

### Accent Colors

```rust
use wgpui::theme::accent;

accent::PRIMARY  // #FFB400 - Bloomberg yellow
accent::BLUE     // #4A9EFF - Links, info
accent::GREEN    // #00C853 - Success
accent::RED      // #D32F2F - Error
accent::PURPLE   // #9C4DCC - Special
```

### Border Colors

```rust
use wgpui::theme::border;

border::DEFAULT  // #1A1A1A - Standard borders
border::SUBTLE   // #141414 - Subtle borders
border::FOCUS    // #FFB400 @ 50% - Focus rings
border::ERROR    // #D32F2F @ 50% - Error borders
```

### Status Colors

```rust
use wgpui::theme::status;

status::SUCCESS  // #00C853 - Completed, success
status::ERROR    // #D32F2F - Failed, error
status::WARNING  // #FFB400 - Warning
status::RUNNING  // #FFB400 - In progress
status::INFO     // #4A9EFF - Informational
```

## Typography

### Font Sizes

```rust
use wgpui::theme::font_size;

font_size::XS    // 11px - Labels, badges
font_size::SM    // 12px - Secondary text
font_size::BASE  // 13px - Default
font_size::LG    // 14px - Section headers
font_size::XL    // 16px - Page titles
font_size::XXL   // 20px - Hero text
```

### Line Heights

```rust
use wgpui::theme::line_height;

line_height::TIGHT    // 1.2 - Compact
line_height::NORMAL   // 1.3 - Default
line_height::RELAXED  // 1.5 - Body text
```

## Spacing

### Spacing Scale

```rust
use wgpui::theme::spacing;

spacing::XS   // 4px
spacing::SM   // 8px
spacing::MD   // 12px
spacing::LG   // 16px
spacing::XL   // 24px
spacing::XXL  // 32px
```

### Border Radius

```rust
use wgpui::theme::radius;

radius::NONE     // 0px
radius::SM       // 2px
radius::DEFAULT  // 4px
radius::MD       // 6px
radius::LG       // 8px
radius::FULL     // 9999px - Pills
```

## Color Type

Colors use the `Hsla` type for GPU-friendly representation:

```rust
use wgpui::Hsla;

// Create from HSLA values (h: 0-1, s: 0-1, l: 0-1, a: 0-1)
let color = Hsla::new(0.117, 1.0, 0.5, 1.0);  // #FFB400

// Create from hex
let color = Hsla::from_hex(0xFFB400);

// Create from RGB
let color = Hsla::from_rgb(1.0, 0.7, 0.0);

// Modify
let lighter = color.lighten(0.2);
let darker = color.darken(0.2);
let faded = color.with_alpha(0.5);

// Convert to RGBA for GPU
let rgba: [f32; 4] = color.to_rgba();
```

## Usage Examples

### Card Component

```rust
scene.draw_quad(
    Quad::new(bounds)
        .with_background(theme::bg::SURFACE)
        .with_border(theme::border::DEFAULT, 1.0)
        .with_uniform_radius(theme::radius::LG)
);
```

### Button

```rust
// Normal state
let bg = theme::accent::PRIMARY;

// Hover state
let bg = theme::accent::PRIMARY.lighten(0.1);

// Pressed state
let bg = theme::accent::PRIMARY.darken(0.1);

scene.draw_quad(
    Quad::new(bounds)
        .with_background(bg)
        .with_uniform_radius(theme::radius::DEFAULT)
);

let text = text_system.layout(
    "Click me",
    bounds.center(),
    theme::font_size::BASE,
    theme::bg::APP  // Dark text on accent
);
```

### Status Indicator

```rust
let color = match status {
    Status::Success => theme::status::SUCCESS,
    Status::Error => theme::status::ERROR,
    Status::Running => theme::status::RUNNING,
    Status::Pending => theme::text::MUTED,
};

scene.draw_quad(
    Quad::new(Bounds::new(x, y, 8.0, 8.0))
        .with_background(color)
        .with_uniform_radius(theme::radius::FULL)
);
```

### Text Hierarchy

```rust
// Title
let title = text_system.layout(
    "Section Title",
    Point::new(x, y),
    theme::font_size::LG,
    theme::accent::PRIMARY
);

// Body
let body = text_system.layout(
    "Description text here",
    Point::new(x, y + 24.0),
    theme::font_size::BASE,
    theme::text::PRIMARY
);

// Caption
let caption = text_system.layout(
    "Last updated: 5m ago",
    Point::new(x, y + 48.0),
    theme::font_size::SM,
    theme::text::MUTED
);
```

## Customization

### Creating Custom Tokens

```rust
mod my_theme {
    use wgpui::Hsla;

    pub mod brand {
        use super::*;
        pub const PRIMARY: Hsla = Hsla::new(0.6, 0.8, 0.5, 1.0);
        pub const SECONDARY: Hsla = Hsla::new(0.3, 0.7, 0.4, 1.0);
    }
}
```

### Runtime Theme Switching

```rust
struct ThemeColors {
    bg_app: Hsla,
    bg_surface: Hsla,
    text_primary: Hsla,
    accent: Hsla,
}

impl ThemeColors {
    fn dark() -> Self {
        Self {
            bg_app: theme::bg::APP,
            bg_surface: theme::bg::SURFACE,
            text_primary: theme::text::PRIMARY,
            accent: theme::accent::PRIMARY,
        }
    }

    fn light() -> Self {
        Self {
            bg_app: Hsla::white(),
            bg_surface: Hsla::from_hex(0xF5F5F5),
            text_primary: Hsla::from_hex(0x1A1A1A),
            accent: theme::accent::PRIMARY,
        }
    }
}
```
