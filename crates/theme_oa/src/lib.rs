//! Centralized theme for Commander UI - Bloomberg Terminal inspired
//!
//! Design Philosophy: "Speed is the product. Everything else is noise."
//! - High density, dark theme
//! - Semantic colors only (no decoration)
//! - Tight typography (11px base, ±1 step)
//! - Berkeley Mono for alignment
//!
//! # Usage
//!
//! ```rust
//! use theme_oa::{bg, text, border, FONT_FAMILY, FONT_SIZE, LINE_HEIGHT};
//!
//! div()
//!     .bg(bg::SURFACE)
//!     .text_color(text::PRIMARY)
//!     .border_color(border::DEFAULT)
//!     .font_family(FONT_FAMILY)
//!     .text_size(px(FONT_SIZE))
//!     .line_height(relative(LINE_HEIGHT))
//! ```

use gpui::Hsla;

// =============================================================================
// TYPOGRAPHY - Bloomberg style: tight, consistent, monospace
// =============================================================================

/// The standard font family - monospace for column alignment
pub const FONT_FAMILY: &str = "Berkeley Mono";

/// Base font size in pixels (Bloomberg uses ~10-11px)
pub const FONT_SIZE: f32 = 11.0;

/// Small font size (-1 step)
pub const FONT_SIZE_SM: f32 = 10.0;

/// Large font size (+1 step, for headers only)
pub const FONT_SIZE_LG: f32 = 12.0;

/// Extra small font size (labels, hints)
pub const FONT_SIZE_XS: f32 = 9.0;

/// Line height - tight for density (Bloomberg uses ~1.1-1.2)
pub const LINE_HEIGHT: f32 = 1.15;

/// Relaxed line height for readable text blocks
pub const LINE_HEIGHT_RELAXED: f32 = 1.3;

// =============================================================================
// HELPER
// =============================================================================

/// Helper to create const Hsla values
const fn c(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla { h, s, l, a }
}

/// Convert hex-ish values to HSL (approximate)
/// Bloomberg colors converted to HSL:
/// #000000 = h:0, s:0, l:0
/// #0A0A0A = h:0, s:0, l:0.04
/// #1A1A1A = h:0, s:0, l:0.10
/// #E6E6E6 = h:0, s:0, l:0.90
/// #B0B0B0 = h:0, s:0, l:0.69
/// #9E9E9E = h:0, s:0, l:0.62
/// #FFB400 = h:42°/360=0.117, s:1.0, l:0.50 (Bloomberg Yellow)
/// #00C853 = h:145°/360=0.403, s:1.0, l:0.39 (Positive Green)
/// #D32F2F = h:0°, s:0.64, l:0.51 (Negative Red)
/// #2979FF = h:218°/360=0.606, s:1.0, l:0.58 (Link Blue)
/// #FF6F00 = h:26°/360=0.072, s:1.0, l:0.50 (Alert Orange)

// =============================================================================
// BACKGROUND COLORS - Pure black base, minimal elevation
// =============================================================================

pub mod bg {
    use super::*;

    /// Pure black - main app background (#000000)
    pub const APP: Hsla = c(0.0, 0.0, 0.0, 1.0);

    /// Near black - surface background (#0A0A0A)
    pub const SURFACE: Hsla = c(0.0, 0.0, 0.04, 1.0);

    /// Elevated surface (#1A1A1A)
    pub const ELEVATED: Hsla = c(0.0, 0.0, 0.10, 1.0);

    /// Card/panel background (#1C1C1C)
    pub const CARD: Hsla = c(0.0, 0.0, 0.11, 1.0);

    /// Hover state background (#1C1C1C)
    pub const HOVER: Hsla = c(0.0, 0.0, 0.11, 1.0);

    /// Selected item background (#1C1C1C)
    pub const SELECTED: Hsla = c(0.0, 0.0, 0.11, 1.0);

    /// Darker surface for contrast (#050505)
    pub const DARK: Hsla = c(0.0, 0.0, 0.02, 1.0);

    /// Panel background (#0A0A0A)
    pub const PANEL: Hsla = c(0.0, 0.0, 0.04, 1.0);

    /// Row background (subtle)
    pub const ROW: Hsla = c(0.0, 0.0, 0.06, 1.0);

    /// Header background (#101010)
    pub const HEADER: Hsla = c(0.0, 0.0, 0.06, 1.0);

    /// Header hover background (#1A1A1A)
    pub const HEADER_HOVER: Hsla = c(0.0, 0.0, 0.10, 1.0);

    /// Subtle row background (#080808)
    pub const ROW_SUBTLE: Hsla = c(0.0, 0.0, 0.03, 1.0);

    /// Sidebar background (#0A0A0A)
    pub const SIDEBAR: Hsla = c(0.0, 0.0, 0.04, 1.0);

    /// Code block background (#101010)
    pub const CODE: Hsla = c(0.0, 0.0, 0.06, 1.0);
}

// =============================================================================
// BORDER COLORS - Subtle, functional
// =============================================================================

pub mod border {
    use super::*;

    /// Default border (#1A1A1A)
    pub const DEFAULT: Hsla = c(0.0, 0.0, 0.10, 1.0);

    /// Subtle border
    pub const SUBTLE: Hsla = c(0.0, 0.0, 0.08, 1.0);

    /// Strong border (#2A2A2A)
    pub const STRONG: Hsla = c(0.0, 0.0, 0.165, 1.0);

    /// Selected item border
    pub const SELECTED: Hsla = c(0.0, 0.0, 0.30, 1.0);

    /// Focus ring - Bloomberg yellow
    pub const FOCUS: Hsla = c(0.117, 1.0, 0.50, 1.0);
}

// =============================================================================
// TEXT COLORS - High contrast, semantic
// =============================================================================

pub mod text {
    use super::*;

    /// Primary text - main content (#E6E6E6)
    pub const PRIMARY: Hsla = c(0.0, 0.0, 0.90, 1.0);

    /// Bright text - values, data (#FFFFFF)
    pub const BRIGHT: Hsla = c(0.0, 0.0, 1.0, 1.0);

    /// Secondary text - less emphasis (#B0B0B0)
    pub const SECONDARY: Hsla = c(0.0, 0.0, 0.69, 1.0);

    /// Muted text - labels, hints (#9E9E9E)
    pub const MUTED: Hsla = c(0.0, 0.0, 0.62, 1.0);

    /// Disabled text (#505050)
    pub const DISABLED: Hsla = c(0.0, 0.0, 0.31, 1.0);

    /// Placeholder text (#9E9E9E)
    pub const PLACEHOLDER: Hsla = c(0.0, 0.0, 0.62, 1.0);

    /// Dim text - alias for muted (#9E9E9E)
    pub const DIM: Hsla = c(0.0, 0.0, 0.62, 1.0);

    /// Link text color - Bloomberg blue (#2979FF)
    pub const LINK: Hsla = c(0.606, 1.0, 0.58, 1.0);

    /// Bloomberg yellow - highlights, your messages (#FFB400)
    pub const HIGHLIGHT: Hsla = c(0.117, 1.0, 0.50, 1.0);
}

// =============================================================================
// SEMANTIC/STATUS COLORS - Bloomberg market colors
// =============================================================================

pub mod status {
    use super::*;

    // Positive/Up - Green (#00C853)
    pub const SUCCESS: Hsla = c(0.403, 1.0, 0.39, 1.0);
    pub const SUCCESS_BG: Hsla = c(0.403, 0.50, 0.15, 0.4);
    pub const SUCCESS_BORDER: Hsla = c(0.403, 0.70, 0.30, 0.5);

    // Negative/Down - Red (#D32F2F)
    pub const ERROR: Hsla = c(0.0, 0.64, 0.51, 1.0);
    pub const ERROR_BG: Hsla = c(0.0, 0.50, 0.20, 0.4);
    pub const ERROR_BORDER: Hsla = c(0.0, 0.60, 0.40, 0.5);

    // Warning - Orange (#FF6F00)
    pub const WARNING: Hsla = c(0.072, 1.0, 0.50, 1.0);
    pub const WARNING_BG: Hsla = c(0.072, 0.50, 0.20, 0.4);
    pub const WARNING_BORDER: Hsla = c(0.072, 0.70, 0.40, 0.5);

    // Info - Blue (#2979FF)
    pub const INFO: Hsla = c(0.606, 1.0, 0.58, 1.0);
    pub const INFO_BG: Hsla = c(0.606, 0.50, 0.20, 0.4);
    pub const INFO_BORDER: Hsla = c(0.606, 0.70, 0.40, 0.5);

    // Running - Yellow (#FFB400)
    pub const RUNNING: Hsla = c(0.117, 1.0, 0.50, 1.0);
    pub const RUNNING_BG: Hsla = c(0.117, 0.50, 0.20, 0.4);

    // Pending - Gray (#9E9E9E)
    pub const PENDING: Hsla = c(0.0, 0.0, 0.62, 1.0);
    pub const PENDING_BG: Hsla = c(0.0, 0.0, 0.20, 0.4);

    // Neutral/Unchanged - Gray (#9E9E9E)
    pub const NEUTRAL: Hsla = c(0.0, 0.0, 0.62, 1.0);
}

// =============================================================================
// ACCENT COLORS - Minimal, semantic only
// =============================================================================

pub mod accent {
    use super::*;

    /// Bloomberg yellow (#FFB400) - primary highlight
    pub const PRIMARY: Hsla = c(0.117, 1.0, 0.50, 1.0);

    /// Muted yellow - subtle primary backgrounds
    pub const PRIMARY_MUTED: Hsla = c(0.117, 0.50, 0.25, 0.4);

    /// Secondary violet (#7E57C2) - tools, actions
    pub const SECONDARY: Hsla = c(0.72, 0.46, 0.55, 1.0);

    /// Muted violet - subtle secondary backgrounds
    pub const SECONDARY_MUTED: Hsla = c(0.72, 0.30, 0.25, 0.4);

    /// Blue (#2979FF) - links, navigation
    pub const BLUE: Hsla = c(0.606, 1.0, 0.58, 1.0);

    /// Green (#00C853) - positive
    pub const GREEN: Hsla = c(0.403, 1.0, 0.39, 1.0);

    /// Tertiary emerald (#10B981) - observation, status
    pub const TERTIARY: Hsla = c(0.437, 0.84, 0.40, 1.0);

    /// Red (#D32F2F) - negative
    pub const RED: Hsla = c(0.0, 0.64, 0.51, 1.0);

    /// Orange (#FF6F00) - alerts
    pub const ORANGE: Hsla = c(0.072, 1.0, 0.50, 1.0);

    /// Purple - rare, special indicators (#7E57C2)
    pub const PURPLE: Hsla = c(0.72, 0.46, 0.55, 1.0);
}

// =============================================================================
// SOURCE BADGES
// =============================================================================

pub mod source {
    use super::*;

    // User - Yellow (Bloomberg highlight)
    pub const USER_BG: Hsla = c(0.117, 0.50, 0.20, 0.4);
    pub const USER_TEXT: Hsla = c(0.117, 1.0, 0.50, 1.0);
    pub const USER_BORDER: Hsla = c(0.117, 0.70, 0.40, 0.5);

    // Agent - Green
    pub const AGENT_BG: Hsla = c(0.403, 0.50, 0.15, 0.4);
    pub const AGENT_TEXT: Hsla = c(0.403, 1.0, 0.39, 1.0);
    pub const AGENT_BORDER: Hsla = c(0.403, 0.70, 0.30, 0.5);

    // System - Gray
    pub const SYSTEM_BG: Hsla = c(0.0, 0.0, 0.15, 0.4);
    pub const SYSTEM_TEXT: Hsla = c(0.0, 0.0, 0.62, 1.0);
    pub const SYSTEM_BORDER: Hsla = c(0.0, 0.0, 0.30, 0.5);
}

// =============================================================================
// TEST CATEGORY BADGES
// =============================================================================

pub mod category {
    use super::*;

    // AntiCheat - Red
    pub const ANTI_CHEAT_BG: Hsla = c(0.0, 0.50, 0.20, 0.4);
    pub const ANTI_CHEAT_TEXT: Hsla = c(0.0, 0.64, 0.51, 1.0);
    pub const ANTI_CHEAT_BORDER: Hsla = c(0.0, 0.60, 0.40, 0.5);

    // Existence - Blue
    pub const EXISTENCE_BG: Hsla = c(0.606, 0.50, 0.20, 0.4);
    pub const EXISTENCE_TEXT: Hsla = c(0.606, 1.0, 0.58, 1.0);
    pub const EXISTENCE_BORDER: Hsla = c(0.606, 0.70, 0.40, 0.5);

    // Correctness - Green
    pub const CORRECTNESS_BG: Hsla = c(0.403, 0.50, 0.15, 0.4);
    pub const CORRECTNESS_TEXT: Hsla = c(0.403, 1.0, 0.39, 1.0);
    pub const CORRECTNESS_BORDER: Hsla = c(0.403, 0.70, 0.30, 0.5);

    // Boundary - Yellow
    pub const BOUNDARY_BG: Hsla = c(0.117, 0.50, 0.20, 0.4);
    pub const BOUNDARY_TEXT: Hsla = c(0.117, 1.0, 0.50, 1.0);
    pub const BOUNDARY_BORDER: Hsla = c(0.117, 0.70, 0.40, 0.5);

    // Integration - Purple
    pub const INTEGRATION_BG: Hsla = c(0.72, 0.30, 0.20, 0.4);
    pub const INTEGRATION_TEXT: Hsla = c(0.72, 0.46, 0.55, 1.0);
    pub const INTEGRATION_BORDER: Hsla = c(0.72, 0.40, 0.40, 0.5);

    // Unknown - Gray
    pub const UNKNOWN_BG: Hsla = c(0.0, 0.0, 0.15, 0.4);
    pub const UNKNOWN_TEXT: Hsla = c(0.0, 0.0, 0.62, 1.0);
    pub const UNKNOWN_BORDER: Hsla = c(0.0, 0.0, 0.30, 0.5);
}

// =============================================================================
// HUD-SPECIFIC COLORS
// =============================================================================

pub mod hud {
    use super::*;

    // Pin states
    pub const PIN_EMPTY: Hsla = c(0.0, 0.0, 0.31, 1.0);
    pub const PIN_VALID: Hsla = c(0.403, 1.0, 0.39, 1.0);
    pub const PIN_INVALID: Hsla = c(0.0, 0.64, 0.51, 1.0);
    pub const PIN_CONSTANT: Hsla = c(0.606, 1.0, 0.58, 1.0);

    // Connection states
    pub const CONNECTION_ACTIVE: Hsla = c(0.0, 0.0, 1.0, 0.8);
    pub const CONNECTION_INACTIVE: Hsla = c(0.0, 0.0, 0.50, 0.5);
    pub const CONNECTION_SELECTED: Hsla = c(0.117, 1.0, 0.50, 1.0);

    // Unit states
    pub const UNIT_PLAYING_BG: Hsla = c(0.0, 0.0, 0.10, 0.95);
    pub const UNIT_PAUSED_BG: Hsla = c(0.0, 0.0, 0.06, 0.95);
    pub const UNIT_ERROR_BG: Hsla = c(0.0, 0.30, 0.15, 0.95);

    // Graph view
    pub const GRAPH_BG: Hsla = c(0.0, 0.0, 0.0, 1.0);
    pub const GRID: Hsla = c(0.0, 0.0, 0.10, 1.0);
    pub const RUBBER_BAND_FILL: Hsla = c(0.117, 0.50, 0.50, 0.15);
    pub const RUBBER_BAND_STROKE: Hsla = c(0.117, 1.0, 0.50, 0.8);

    // APM widget - Bloomberg yellow based
    pub const APM_WIDGET_BG: Hsla = c(0.0, 0.0, 0.04, 0.95);
    pub const APM_WIDGET_BORDER: Hsla = c(0.117, 0.50, 0.50, 0.25);

    // APM velocity levels
    pub const APM_BASELINE: Hsla = c(0.0, 0.0, 0.46, 1.0);
    pub const APM_ACTIVE: Hsla = c(0.606, 1.0, 0.58, 1.0);
    pub const APM_HIGH: Hsla = c(0.403, 1.0, 0.39, 1.0);
    pub const APM_ELITE: Hsla = c(0.117, 1.0, 0.50, 1.0);

    // Task status - Bloomberg semantic colors
    pub const STATUS_PENDING: Hsla = c(0.0, 0.0, 0.62, 1.0);
    pub const STATUS_RUNNING: Hsla = c(0.117, 1.0, 0.50, 1.0);
    pub const STATUS_PASSED: Hsla = c(0.403, 1.0, 0.39, 1.0);
    pub const STATUS_FAILED: Hsla = c(0.0, 0.64, 0.51, 1.0);
    pub const STATUS_TIMEOUT: Hsla = c(0.072, 1.0, 0.50, 1.0);
    pub const STATUS_ERROR: Hsla = c(0.0, 0.64, 0.51, 1.0);

    // Difficulty badges
    pub const DIFFICULTY_EASY_BG: Hsla = c(0.403, 0.50, 0.15, 0.4);
    pub const DIFFICULTY_EASY_TEXT: Hsla = c(0.403, 1.0, 0.39, 1.0);
    pub const DIFFICULTY_MEDIUM_BG: Hsla = c(0.117, 0.50, 0.20, 0.4);
    pub const DIFFICULTY_MEDIUM_TEXT: Hsla = c(0.117, 1.0, 0.50, 1.0);
    pub const DIFFICULTY_HARD_BG: Hsla = c(0.0, 0.50, 0.20, 0.4);
    pub const DIFFICULTY_HARD_TEXT: Hsla = c(0.0, 0.64, 0.51, 1.0);
    pub const DIFFICULTY_UNKNOWN_BG: Hsla = c(0.0, 0.0, 0.15, 0.4);
    pub const DIFFICULTY_UNKNOWN_TEXT: Hsla = c(0.0, 0.0, 0.62, 1.0);

    // Panel/container
    pub const PANEL_BG: Hsla = c(0.0, 0.0, 0.04, 0.95);
    pub const PANEL_BORDER: Hsla = c(0.0, 0.0, 0.10, 0.6);
    pub const HEADER_BG: Hsla = c(0.0, 0.0, 0.06, 0.8);
    pub const DIVIDER: Hsla = c(0.0, 0.0, 0.10, 0.4);

    // Buttons
    pub const BUTTON_BORDER: Hsla = c(0.0, 0.0, 0.20, 0.5);
    pub const BUTTON_BORDER_HOVER: Hsla = c(0.0, 0.0, 0.30, 0.6);

    // Rows
    pub const ROW_HOVER: Hsla = c(0.0, 0.0, 0.08, 0.4);
    pub const ROW_SELECTED: Hsla = c(0.0, 0.0, 0.11, 0.6);
}

// =============================================================================
// SYNTAX HIGHLIGHTING - High contrast for readability
// =============================================================================

pub mod syntax {
    use super::*;

    pub const KEYWORD: Hsla = c(0.606, 1.0, 0.58, 1.0);     // Blue
    pub const STRING: Hsla = c(0.403, 1.0, 0.39, 1.0);      // Green
    pub const NUMBER: Hsla = c(0.117, 1.0, 0.50, 1.0);      // Yellow
    pub const COMMENT: Hsla = c(0.0, 0.0, 0.50, 1.0);       // Gray
    pub const FUNCTION: Hsla = c(0.072, 1.0, 0.50, 1.0);    // Orange
    pub const TYPE: Hsla = c(0.606, 1.0, 0.58, 1.0);        // Blue
    pub const VARIABLE: Hsla = c(0.0, 0.0, 0.90, 1.0);      // White
    pub const PROPERTY: Hsla = c(0.72, 0.46, 0.55, 1.0);    // Purple
}

// =============================================================================
// INPUT COLORS
// =============================================================================

pub mod input {
    use super::*;

    pub const PLACEHOLDER: Hsla = c(0.0, 0.0, 0.62, 1.0);
    pub const TEXT: Hsla = c(0.0, 0.0, 0.90, 1.0);
    pub const CURSOR: Hsla = c(0.117, 1.0, 0.50, 1.0);  // Bloomberg yellow
    pub const SELECTION: Hsla = c(0.0, 0.0, 0.20, 0.60);
    pub const BG: Hsla = c(0.0, 0.0, 0.06, 1.0);
    pub const BORDER: Hsla = c(0.0, 0.0, 0.10, 1.0);
}

// =============================================================================
// TRUST TIER COLORS
// =============================================================================

pub mod trust {
    use super::*;

    // Bronze
    pub const BRONZE: Hsla = c(0.08, 0.60, 0.40, 1.0);
    pub const BRONZE_BG: Hsla = c(0.08, 0.40, 0.20, 0.4);
    pub const BRONZE_BORDER: Hsla = c(0.08, 0.50, 0.35, 0.5);

    // Silver
    pub const SILVER: Hsla = c(0.0, 0.0, 0.75, 1.0);
    pub const SILVER_BG: Hsla = c(0.0, 0.0, 0.40, 0.4);
    pub const SILVER_BORDER: Hsla = c(0.0, 0.0, 0.60, 0.5);

    // Gold - Bloomberg yellow
    pub const GOLD: Hsla = c(0.117, 1.0, 0.50, 1.0);
    pub const GOLD_BG: Hsla = c(0.117, 0.50, 0.25, 0.4);
    pub const GOLD_BORDER: Hsla = c(0.117, 0.70, 0.40, 0.5);

    // Diamond - Blue
    pub const DIAMOND: Hsla = c(0.606, 0.80, 0.70, 1.0);
    pub const DIAMOND_BG: Hsla = c(0.606, 0.50, 0.25, 0.4);
    pub const DIAMOND_BORDER: Hsla = c(0.606, 0.70, 0.50, 0.5);
}

// =============================================================================
// UI COMPONENT COLORS - Grayscale with semantic accents
// =============================================================================

pub mod ui {
    use super::*;

    pub mod button {
        use super::*;

        // Default - white/light for primary action
        pub const DEFAULT_BG: Hsla = c(0.0, 0.0, 0.90, 1.0);
        pub const DEFAULT_TEXT: Hsla = c(0.0, 0.0, 0.0, 1.0);
        pub const DEFAULT_HOVER_BG: Hsla = c(0.0, 0.0, 0.80, 1.0);

        // Destructive - red
        pub const DESTRUCTIVE_BG: Hsla = c(0.0, 0.64, 0.51, 1.0);
        pub const DESTRUCTIVE_TEXT: Hsla = c(0.0, 0.0, 1.0, 1.0);
        pub const DESTRUCTIVE_HOVER_BG: Hsla = c(0.0, 0.64, 0.45, 1.0);

        // Outline
        pub const OUTLINE_BG: Hsla = c(0.0, 0.0, 0.0, 0.0);
        pub const OUTLINE_TEXT: Hsla = c(0.0, 0.0, 0.90, 1.0);
        pub const OUTLINE_BORDER: Hsla = c(0.0, 0.0, 0.20, 1.0);
        pub const OUTLINE_HOVER_BG: Hsla = c(0.0, 0.0, 0.10, 1.0);

        // Secondary
        pub const SECONDARY_BG: Hsla = c(0.0, 0.0, 0.10, 1.0);
        pub const SECONDARY_TEXT: Hsla = c(0.0, 0.0, 0.90, 1.0);
        pub const SECONDARY_HOVER_BG: Hsla = c(0.0, 0.0, 0.15, 1.0);

        // Ghost
        pub const GHOST_BG: Hsla = c(0.0, 0.0, 0.0, 0.0);
        pub const GHOST_TEXT: Hsla = c(0.0, 0.0, 0.90, 1.0);
        pub const GHOST_HOVER_BG: Hsla = c(0.0, 0.0, 0.10, 1.0);

        // Link
        pub const LINK_TEXT: Hsla = c(0.606, 1.0, 0.58, 1.0);
        pub const LINK_HOVER_TEXT: Hsla = c(0.606, 1.0, 0.70, 1.0);
    }

    pub mod checkbox {
        use super::*;

        pub const UNCHECKED_BG: Hsla = c(0.0, 0.0, 0.0, 0.0);
        pub const UNCHECKED_BORDER: Hsla = c(0.0, 0.0, 0.40, 1.0);
        pub const CHECKED_BG: Hsla = c(0.0, 0.0, 0.90, 1.0);
        pub const CHECKED_BORDER: Hsla = c(0.0, 0.0, 0.90, 1.0);
        pub const CHECK_ICON: Hsla = c(0.0, 0.0, 0.0, 1.0);
    }

    pub mod switch {
        use super::*;

        pub const TRACK_OFF: Hsla = c(0.0, 0.0, 0.20, 1.0);
        pub const TRACK_ON: Hsla = c(0.0, 0.0, 0.90, 1.0);
        pub const THUMB: Hsla = c(0.0, 0.0, 1.0, 1.0);
        pub const THUMB_ON: Hsla = c(0.0, 0.0, 0.0, 1.0);
    }

    pub mod progress {
        use super::*;

        pub const TRACK_BG: Hsla = c(0.0, 0.0, 0.10, 1.0);
        pub const INDICATOR: Hsla = c(0.0, 0.0, 0.90, 1.0);
    }

    pub mod skeleton {
        use super::*;

        pub const BG: Hsla = c(0.0, 0.0, 0.10, 1.0);
        pub const SHIMMER: Hsla = c(0.0, 0.0, 0.15, 1.0);
    }

    pub mod separator {
        use super::*;

        pub const DEFAULT: Hsla = c(0.0, 0.0, 0.10, 1.0);
    }

    pub mod label {
        use super::*;

        pub const TEXT: Hsla = c(0.0, 0.0, 0.90, 1.0);
        pub const DISABLED: Hsla = c(0.0, 0.0, 0.40, 1.0);
    }

    pub mod kbd {
        use super::*;

        pub const BG: Hsla = c(0.0, 0.0, 0.10, 1.0);
        pub const BORDER: Hsla = c(0.0, 0.0, 0.20, 1.0);
        pub const TEXT: Hsla = c(0.0, 0.0, 0.80, 1.0);
    }
}
