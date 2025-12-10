# Plan: Centralize UI Theme Colors

## Problem

110+ inline color definitions (`hsla()`, `rgb()`, hex) scattered across:
- `crates/commander/` - 50+ inline calls
- `crates/storybook/` - 40+ inline calls
- `crates/hud/` - 19 inline calls

Same colors are duplicated everywhere with slight variations.

## Solution

Create a dedicated `crates/theme/` crate with all color constants, then refactor all components to import from it.

---

## Phase 1: Create Theme Crate

### 1.1 Create `crates/theme/Cargo.toml`

```toml
[package]
name = "theme"
version = "0.1.0"
edition = "2024"
description = "Centralized UI theme colors for OpenAgents"
license = "MIT"

[dependencies]
gpui = { path = "../gpui" }
```

### 1.2 Create `crates/theme/src/lib.rs`

```rust
//! Centralized theme colors for Commander UI
//!
//! All color definitions should go here. Never use inline hsla()/rgb() in components.

use gpui::{hsla, Hsla};

/// Background colors
pub mod bg {
    use super::*;
    pub const APP: Hsla = hsla(0.0, 0.0, 0.0, 1.0);           // Pure black
    pub const SURFACE: Hsla = hsla(0.0, 0.0, 0.05, 1.0);      // Near black
    pub const ELEVATED: Hsla = hsla(0.0, 0.0, 0.08, 0.6);     // Slightly elevated
    pub const CARD: Hsla = hsla(0.0, 0.0, 0.12, 0.4);         // Card background
    pub const HOVER: Hsla = hsla(0.0, 0.0, 0.15, 0.6);        // Hover state
    pub const SELECTED: Hsla = hsla(0.58, 0.5, 0.15, 0.3);    // Selected item (cyan tint)
}

/// Border colors
pub mod border {
    use super::*;
    pub const DEFAULT: Hsla = hsla(0.0, 0.0, 0.2, 0.4);
    pub const SUBTLE: Hsla = hsla(0.0, 0.0, 0.15, 0.4);
    pub const STRONG: Hsla = hsla(0.0, 0.0, 0.3, 0.6);
    pub const SELECTED: Hsla = hsla(0.58, 0.5, 0.35, 0.5);    // Cyan
}

/// Text colors
pub mod text {
    use super::*;
    pub const PRIMARY: Hsla = hsla(0.0, 0.0, 0.9, 1.0);       // Main text
    pub const SECONDARY: Hsla = hsla(0.0, 0.0, 0.7, 1.0);     // Less emphasis
    pub const MUTED: Hsla = hsla(0.0, 0.0, 0.5, 1.0);         // Labels, hints
    pub const DISABLED: Hsla = hsla(0.0, 0.0, 0.4, 1.0);      // Disabled state
    pub const PLACEHOLDER: Hsla = hsla(0.0, 0.0, 0.4, 1.0);   // Placeholder text
}

/// Semantic/status colors
pub mod status {
    use super::*;
    // Success/valid - green
    pub const SUCCESS: Hsla = hsla(0.38, 0.6, 0.5, 1.0);
    pub const SUCCESS_BG: Hsla = hsla(0.38, 0.5, 0.2, 0.4);

    // Error/invalid - red
    pub const ERROR: Hsla = hsla(0.0, 0.6, 0.5, 1.0);
    pub const ERROR_BG: Hsla = hsla(0.0, 0.5, 0.2, 0.4);

    // Warning - yellow/orange
    pub const WARNING: Hsla = hsla(0.15, 0.6, 0.5, 1.0);
    pub const WARNING_BG: Hsla = hsla(0.15, 0.5, 0.2, 0.4);

    // Info - blue
    pub const INFO: Hsla = hsla(0.58, 0.6, 0.5, 1.0);
    pub const INFO_BG: Hsla = hsla(0.58, 0.5, 0.2, 0.4);
}

/// Accent/brand colors
pub mod accent {
    use super::*;
    pub const PRIMARY: Hsla = hsla(0.58, 0.8, 0.6, 1.0);      // Cyan - selection, links
    pub const SECONDARY: Hsla = hsla(0.78, 0.6, 0.6, 1.0);    // Violet - tool calls
    pub const TERTIARY: Hsla = hsla(0.38, 0.6, 0.6, 1.0);     // Emerald - agent
}

/// Source badges (ATIF step sources)
pub mod source {
    use super::*;
    // User - blue
    pub const USER_BG: Hsla = hsla(0.58, 0.5, 0.2, 0.4);
    pub const USER_TEXT: Hsla = hsla(0.58, 0.6, 0.7, 1.0);
    pub const USER_BORDER: Hsla = hsla(0.58, 0.5, 0.35, 0.5);

    // Agent - green
    pub const AGENT_BG: Hsla = hsla(0.38, 0.5, 0.2, 0.4);
    pub const AGENT_TEXT: Hsla = hsla(0.38, 0.6, 0.7, 1.0);
    pub const AGENT_BORDER: Hsla = hsla(0.38, 0.5, 0.35, 0.5);

    // System - gray
    pub const SYSTEM_BG: Hsla = hsla(0.0, 0.0, 0.2, 0.4);
    pub const SYSTEM_TEXT: Hsla = hsla(0.0, 0.0, 0.7, 1.0);
    pub const SYSTEM_BORDER: Hsla = hsla(0.0, 0.0, 0.35, 0.5);
}

/// Test category badges
pub mod category {
    use super::*;
    // AntiCheat - red
    pub const ANTI_CHEAT_BG: Hsla = hsla(0.0, 0.5, 0.3, 0.4);
    pub const ANTI_CHEAT_TEXT: Hsla = hsla(0.0, 0.7, 0.7, 1.0);

    // Existence - blue
    pub const EXISTENCE_BG: Hsla = hsla(0.58, 0.5, 0.3, 0.4);
    pub const EXISTENCE_TEXT: Hsla = hsla(0.58, 0.7, 0.7, 1.0);

    // Correctness - green
    pub const CORRECTNESS_BG: Hsla = hsla(0.38, 0.5, 0.3, 0.4);
    pub const CORRECTNESS_TEXT: Hsla = hsla(0.38, 0.7, 0.7, 1.0);

    // Boundary - yellow
    pub const BOUNDARY_BG: Hsla = hsla(0.15, 0.5, 0.3, 0.4);
    pub const BOUNDARY_TEXT: Hsla = hsla(0.15, 0.7, 0.7, 1.0);

    // Integration - purple
    pub const INTEGRATION_BG: Hsla = hsla(0.78, 0.5, 0.3, 0.4);
    pub const INTEGRATION_TEXT: Hsla = hsla(0.78, 0.7, 0.7, 1.0);
}

/// HUD-specific colors (pins, units, connections)
pub mod hud {
    use super::*;
    // Pin states
    pub const PIN_EMPTY: Hsla = hsla(0.0, 0.0, 0.3, 1.0);
    pub const PIN_VALID: Hsla = hsla(0.33, 0.8, 0.5, 1.0);
    pub const PIN_INVALID: Hsla = hsla(0.0, 0.8, 0.5, 1.0);
    pub const PIN_CONSTANT: Hsla = hsla(0.58, 0.8, 0.6, 1.0);

    // Connection states
    pub const CONNECTION_ACTIVE: Hsla = hsla(0.0, 0.0, 1.0, 0.8);
    pub const CONNECTION_INACTIVE: Hsla = hsla(0.0, 0.0, 0.5, 0.5);
    pub const CONNECTION_SELECTED: Hsla = hsla(0.58, 0.8, 0.6, 1.0);

    // Unit states
    pub const UNIT_PLAYING_BG: Hsla = hsla(0.0, 0.0, 0.15, 0.95);
    pub const UNIT_PAUSED_BG: Hsla = hsla(0.0, 0.0, 0.1, 0.95);
    pub const UNIT_ERROR_BG: Hsla = hsla(0.0, 0.5, 0.15, 0.95);

    // Graph view
    pub const GRAPH_BG: Hsla = hsla(0.0, 0.0, 0.05, 1.0);
    pub const GRID: Hsla = hsla(0.0, 0.0, 0.1, 1.0);
    pub const RUBBER_BAND_FILL: Hsla = hsla(0.6, 0.8, 0.5, 0.15);
    pub const RUBBER_BAND_STROKE: Hsla = hsla(0.6, 0.8, 0.5, 0.8);
}
```

---

## Phase 2: Add Theme Dependency to Crates

Add to each crate's `Cargo.toml`:

```toml
theme = { path = "../theme" }
```

**Crates to update:**
- `crates/commander/Cargo.toml`
- `crates/storybook/Cargo.toml`
- `crates/hud/Cargo.toml`

---

## Phase 3: Update Crates to Use Theme

### Commander (50+ replacements)
| File | Changes |
|------|---------|
| `src/main.rs` | `use theme::{bg, text, border, status};` - Replace all inline colors |
| `src/components/step_view.rs` | Replace `source_badge_colors()` with `theme::source::*` |
| `src/components/thread_item.rs` | Replace `badge_colors()` with `theme::category::*` |
| `src/components/trajectory_detail.rs` | Replace all inline colors |
| `src/components/trajectory_list.rs` | Replace status badges with `theme::status::*` |
| `src/text_input.rs` | Replace with `theme::text::PLACEHOLDER` etc |
| `src/markdown.rs` | Replace with `theme::accent::PRIMARY` |

### Storybook (40+ replacements)
| File | Changes |
|------|---------|
| `src/main.rs` | Replace `rgb(0x1e1e1e)` with `theme::bg::APP` |
| `src/story.rs` | Replace all section colors with theme constants |
| `src/stories/*.rs` | Replace all demo colors |

### HUD (19 replacements)
| File | Changes |
|------|---------|
| `src/pin_view.rs` | Use `theme::hud::PIN_*` |
| `src/unit_view.rs` | Use `theme::hud::UNIT_*` |
| `src/connection.rs` | Use `theme::hud::CONNECTION_*` |
| `src/graph_view.rs` | Use `theme::hud::GRAPH_*`, `theme::hud::GRID` |

---

## Files to Create/Modify

| File | Action |
|------|--------|
| `crates/theme/Cargo.toml` | **CREATE** |
| `crates/theme/src/lib.rs` | **CREATE** - All color constants |
| `crates/commander/Cargo.toml` | Add theme dependency |
| `crates/commander/src/main.rs` | Import theme, replace colors |
| `crates/commander/src/components/*.rs` | Import theme, replace colors |
| `crates/storybook/Cargo.toml` | Add theme dependency |
| `crates/storybook/src/main.rs` | Import theme, replace colors |
| `crates/storybook/src/story.rs` | Import theme, replace colors |
| `crates/storybook/src/stories/*.rs` | Import theme, replace colors |
| `crates/hud/Cargo.toml` | Add theme dependency |
| `crates/hud/src/*.rs` | Import theme, replace colors |

---

## Migration Strategy

1. **Create `crates/theme/`** crate with Cargo.toml and lib.rs
2. **Add theme dependency** to commander, storybook, hud
3. **Update commander** - largest crate, do first
4. **Update storybook** - stories and story.rs
5. **Update hud** - pin_view, unit_view, connection, graph_view
6. **Run storybook** to visually verify all colors match
7. **Grep for remaining inline colors** - should be zero

---

## Validation

After migration, this grep should return 0 results (excluding lib.rs in theme crate):
```bash
rg 'hsla\(|rgb\(0x|rgba\(' --type rust crates/commander crates/storybook crates/hud
```

All matches should only be in `crates/theme/src/lib.rs`.
