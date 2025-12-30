# wgpui Theme System

The theme system provides centralized color management for the entire UI. All colors must go through the theme system to ensure consistency and enable future theme switching.

## Quick Start

```rust
use wgpui::theme;

fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
    let colors = theme().colors;

    // Use semantic colors
    let bg = colors.background;      // Main app background
    let text = colors.text;          // Primary text
    let accent = colors.accent;      // Primary accent (yellow)

    // Draw with theme colors
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(bg)
    );
}
```

## Available Colors

### Backgrounds

| Color | Purpose | Midnight Theme |
|-------|---------|----------------|
| `background` | Main app background | Pure black (#000000) |
| `surface` | Cards, panels | Dark gray (#0d0d0d) |
| `elevated` | Modals, popovers | Slightly lighter (#141414) |
| `hover` | Hover states | (#1f1f1f) |
| `selected` | Selected items | (#262626) |

### Text

| Color | Purpose | Midnight Theme |
|-------|---------|----------------|
| `text` | Primary text | Pure white (#ffffff) |
| `text_muted` | Secondary/muted text | Light gray (#b3b3b3) |
| `text_disabled` | Disabled text | Medium gray (#666666) |
| `text_accent` | Accent text, links | Yellow |

### Borders

| Color | Purpose | Midnight Theme |
|-------|---------|----------------|
| `border` | Default borders | (#333333) |
| `border_focused` | Focused element borders | Yellow 50% |
| `border_transparent` | Subtle separators | Very subtle |

### Status

| Color | Purpose | Midnight Theme |
|-------|---------|----------------|
| `success` | Success states | Green |
| `warning` | Warning states | Yellow |
| `error` | Error states | Red |
| `info` | Info states | Blue |

### Accents

| Color | Purpose | Midnight Theme |
|-------|---------|----------------|
| `accent` | Primary accent | Bloomberg yellow (#FFB400) |
| `accent_hover` | Accent hover state | Slightly lighter yellow |

## Color Modifiers

Use `.with_alpha()` to adjust opacity:

```rust
let colors = theme().colors;

// Semi-transparent border
colors.border.with_alpha(0.5)

// Faded text
colors.text.with_alpha(0.7)
```

## Adding New Colors

If you need a color not in the theme, add it to the `ThemeColors` struct:

1. Edit `crates/wgpui/src/theme/mod.rs`
2. Add the field to `ThemeColors` struct
3. Add the color value in `crates/wgpui/src/theme/themes/midnight.rs`
4. Update backwards-compat aliases if needed

## Pre-commit Hook

A pre-commit hook enforces that all colors go through the theme system:

- **Blocked**: `Hsla::new(0.0, 0.0, 0.5, 1.0)` (hardcoded)
- **Allowed**: `theme().colors.text` (through theme)

The hook will reject commits with hardcoded colors and show how to fix them.

### Allowed Locations

Hardcoded colors are only allowed in:
- `crates/wgpui/src/theme/` (theme definitions)
- `crates/wgpui/src/color.rs` (color type implementation)

## Themes

### Midnight (Default)

The "Midnight" theme features:
- Pure black background for maximum contrast
- Pure white text for readability
- Bloomberg-style yellow accents
- Semantic status colors (green/yellow/red/blue)

### Adding a New Theme

1. Create `crates/wgpui/src/theme/themes/your_theme.rs`:

```rust
use crate::color::Hsla;
use super::super::{Theme, ThemeColors};

pub static YOUR_THEME: Theme = Theme {
    name: "Your Theme",
    colors: ThemeColors {
        background: Hsla::new(0.0, 0.0, 0.1, 1.0),
        // ... all other colors
    },
};
```

2. Export in `crates/wgpui/src/theme/themes/mod.rs`:

```rust
mod your_theme;
pub use your_theme::YOUR_THEME;
```

3. Switch themes at runtime:

```rust
use wgpui::{set_theme, theme::themes::YOUR_THEME};

set_theme(&YOUR_THEME);
```

## Migration Guide

### From Hardcoded Colors

Before:
```rust
let line_color = Hsla::new(0.0, 0.0, 0.3, 0.5);
let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.9);
```

After:
```rust
use wgpui::theme;

let colors = theme().colors;
let line_color = colors.border.with_alpha(0.5);
let bg_color = colors.surface.with_alpha(0.9);
```

### From Old Constants

The old `theme::text::PRIMARY` style constants still work for backwards compatibility but are deprecated:

Before (deprecated):
```rust
use wgpui::theme;
let color = theme::text::PRIMARY;
```

After (preferred):
```rust
use wgpui::theme;
let color = theme().colors.text;
```

## Architecture

```
crates/wgpui/src/theme/
├── mod.rs              # Theme, ThemeColors, theme(), backwards-compat aliases
├── themes/
│   ├── mod.rs          # Theme exports
│   └── midnight.rs     # Midnight theme definition
├── color.rs            # ThemeColor type (for color scales)
├── dynamic.rs          # ThemeMultiplier, ThemeUnit, ThemeBreakpoints
├── style.rs            # ThemeStyle
└── builder.rs          # ThemeCreator for complex themes
```

## Best Practices

1. **Always use semantic colors**: Choose colors by meaning, not value
   - `colors.success` not `Hsla::new(120.0, 0.7, 0.5, 1.0)`

2. **Get colors once**: Cache the colors reference at the start of paint
   ```rust
   let colors = theme().colors;  // Get once
   // Use colors.X throughout
   ```

3. **Use with_alpha for variations**: Don't create new colors for opacity changes
   ```rust
   colors.border.with_alpha(0.5)  // Good
   Hsla::new(0.0, 0.0, 0.2, 0.5)  // Bad - hardcoded
   ```

4. **Add missing colors to theme**: If you need a specific color, add it to ThemeColors rather than hardcoding
