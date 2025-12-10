//! Centralized theme colors for Commander UI
//!
//! All color definitions should go here. Never use inline hsla()/rgb() in components.
//!
//! # Usage
//!
//! ```rust
//! use theme::{bg, text, border};
//!
//! div()
//!     .bg(bg::SURFACE)
//!     .text_color(text::PRIMARY)
//!     .border_color(border::DEFAULT)
//! ```

use gpui::{hsla, Hsla};

/// Background colors
pub mod bg {
    use super::*;

    /// Pure black - main app background
    pub const APP: Hsla = hsla(0.0, 0.0, 0.0, 1.0);

    /// Near black - surface background
    pub const SURFACE: Hsla = hsla(0.0, 0.0, 0.05, 1.0);

    /// Slightly elevated surface
    pub const ELEVATED: Hsla = hsla(0.0, 0.0, 0.08, 0.6);

    /// Card/panel background
    pub const CARD: Hsla = hsla(0.0, 0.0, 0.12, 0.4);

    /// Hover state background
    pub const HOVER: Hsla = hsla(0.0, 0.0, 0.15, 0.6);

    /// Selected item background (cyan tint)
    pub const SELECTED: Hsla = hsla(0.58, 0.5, 0.15, 0.3);

    /// Darker surface for contrast
    pub const DARK: Hsla = hsla(0.0, 0.0, 0.03, 1.0);

    /// Code block background
    pub const CODE: Hsla = hsla(0.0, 0.0, 0.08, 1.0);
}

/// Border colors
pub mod border {
    use super::*;

    /// Default border
    pub const DEFAULT: Hsla = hsla(0.0, 0.0, 0.2, 0.4);

    /// Subtle/faint border
    pub const SUBTLE: Hsla = hsla(0.0, 0.0, 0.15, 0.4);

    /// Strong/prominent border
    pub const STRONG: Hsla = hsla(0.0, 0.0, 0.3, 0.6);

    /// Selected item border (cyan)
    pub const SELECTED: Hsla = hsla(0.58, 0.5, 0.35, 0.5);

    /// Focus ring color
    pub const FOCUS: Hsla = hsla(0.58, 0.8, 0.6, 0.8);
}

/// Text colors
pub mod text {
    use super::*;

    /// Primary text - main content
    pub const PRIMARY: Hsla = hsla(0.0, 0.0, 0.9, 1.0);

    /// Secondary text - less emphasis
    pub const SECONDARY: Hsla = hsla(0.0, 0.0, 0.7, 1.0);

    /// Muted text - labels, hints
    pub const MUTED: Hsla = hsla(0.0, 0.0, 0.5, 1.0);

    /// Disabled text
    pub const DISABLED: Hsla = hsla(0.0, 0.0, 0.4, 1.0);

    /// Placeholder text
    pub const PLACEHOLDER: Hsla = hsla(0.0, 0.0, 0.4, 1.0);

    /// Link text color
    pub const LINK: Hsla = hsla(0.58, 0.8, 0.6, 1.0);
}

/// Semantic/status colors
pub mod status {
    use super::*;

    // Success/valid - green
    /// Success text/icon color
    pub const SUCCESS: Hsla = hsla(0.38, 0.6, 0.5, 1.0);
    /// Success background
    pub const SUCCESS_BG: Hsla = hsla(0.38, 0.5, 0.2, 0.4);
    /// Success border
    pub const SUCCESS_BORDER: Hsla = hsla(0.38, 0.5, 0.35, 0.5);

    // Error/invalid - red
    /// Error text/icon color
    pub const ERROR: Hsla = hsla(0.0, 0.6, 0.5, 1.0);
    /// Error background
    pub const ERROR_BG: Hsla = hsla(0.0, 0.5, 0.2, 0.4);
    /// Error border
    pub const ERROR_BORDER: Hsla = hsla(0.0, 0.5, 0.35, 0.5);

    // Warning - yellow/orange
    /// Warning text/icon color
    pub const WARNING: Hsla = hsla(0.15, 0.6, 0.5, 1.0);
    /// Warning background
    pub const WARNING_BG: Hsla = hsla(0.15, 0.5, 0.2, 0.4);
    /// Warning border
    pub const WARNING_BORDER: Hsla = hsla(0.15, 0.5, 0.35, 0.5);

    // Info - blue/cyan
    /// Info text/icon color
    pub const INFO: Hsla = hsla(0.58, 0.6, 0.5, 1.0);
    /// Info background
    pub const INFO_BG: Hsla = hsla(0.58, 0.5, 0.2, 0.4);
    /// Info border
    pub const INFO_BORDER: Hsla = hsla(0.58, 0.5, 0.35, 0.5);

    // Running/in-progress - yellow
    /// Running state text
    pub const RUNNING: Hsla = hsla(0.15, 0.7, 0.6, 1.0);
    /// Running state background
    pub const RUNNING_BG: Hsla = hsla(0.15, 0.5, 0.2, 0.4);

    // Pending - gray
    /// Pending state text
    pub const PENDING: Hsla = hsla(0.0, 0.0, 0.5, 1.0);
    /// Pending state background
    pub const PENDING_BG: Hsla = hsla(0.0, 0.0, 0.2, 0.4);
}

/// Accent/brand colors
pub mod accent {
    use super::*;

    /// Primary accent - cyan (selection, links, focus)
    pub const PRIMARY: Hsla = hsla(0.58, 0.8, 0.6, 1.0);

    /// Secondary accent - violet (tool calls, special actions)
    pub const SECONDARY: Hsla = hsla(0.78, 0.6, 0.6, 1.0);

    /// Tertiary accent - emerald (agent, success indicators)
    pub const TERTIARY: Hsla = hsla(0.38, 0.6, 0.6, 1.0);

    /// Muted cyan for backgrounds
    pub const PRIMARY_MUTED: Hsla = hsla(0.58, 0.5, 0.2, 0.4);

    /// Muted violet for backgrounds
    pub const SECONDARY_MUTED: Hsla = hsla(0.78, 0.5, 0.2, 0.4);
}

/// Source badges (ATIF step sources)
pub mod source {
    use super::*;

    // User - blue/cyan
    /// User source badge background
    pub const USER_BG: Hsla = hsla(0.58, 0.5, 0.2, 0.4);
    /// User source badge text
    pub const USER_TEXT: Hsla = hsla(0.58, 0.6, 0.7, 1.0);
    /// User source badge border
    pub const USER_BORDER: Hsla = hsla(0.58, 0.5, 0.35, 0.5);

    // Agent - green
    /// Agent source badge background
    pub const AGENT_BG: Hsla = hsla(0.38, 0.5, 0.2, 0.4);
    /// Agent source badge text
    pub const AGENT_TEXT: Hsla = hsla(0.38, 0.6, 0.7, 1.0);
    /// Agent source badge border
    pub const AGENT_BORDER: Hsla = hsla(0.38, 0.5, 0.35, 0.5);

    // System - gray
    /// System source badge background
    pub const SYSTEM_BG: Hsla = hsla(0.0, 0.0, 0.2, 0.4);
    /// System source badge text
    pub const SYSTEM_TEXT: Hsla = hsla(0.0, 0.0, 0.7, 1.0);
    /// System source badge border
    pub const SYSTEM_BORDER: Hsla = hsla(0.0, 0.0, 0.35, 0.5);
}

/// Test category badges
pub mod category {
    use super::*;

    // AntiCheat - red
    /// AntiCheat category background
    pub const ANTI_CHEAT_BG: Hsla = hsla(0.0, 0.5, 0.3, 0.4);
    /// AntiCheat category text
    pub const ANTI_CHEAT_TEXT: Hsla = hsla(0.0, 0.7, 0.7, 1.0);
    /// AntiCheat category border
    pub const ANTI_CHEAT_BORDER: Hsla = hsla(0.0, 0.5, 0.4, 0.5);

    // Existence - blue
    /// Existence category background
    pub const EXISTENCE_BG: Hsla = hsla(0.58, 0.5, 0.3, 0.4);
    /// Existence category text
    pub const EXISTENCE_TEXT: Hsla = hsla(0.58, 0.7, 0.7, 1.0);
    /// Existence category border
    pub const EXISTENCE_BORDER: Hsla = hsla(0.58, 0.5, 0.4, 0.5);

    // Correctness - green
    /// Correctness category background
    pub const CORRECTNESS_BG: Hsla = hsla(0.38, 0.5, 0.3, 0.4);
    /// Correctness category text
    pub const CORRECTNESS_TEXT: Hsla = hsla(0.38, 0.7, 0.7, 1.0);
    /// Correctness category border
    pub const CORRECTNESS_BORDER: Hsla = hsla(0.38, 0.5, 0.4, 0.5);

    // Boundary - yellow
    /// Boundary category background
    pub const BOUNDARY_BG: Hsla = hsla(0.15, 0.5, 0.3, 0.4);
    /// Boundary category text
    pub const BOUNDARY_TEXT: Hsla = hsla(0.15, 0.7, 0.7, 1.0);
    /// Boundary category border
    pub const BOUNDARY_BORDER: Hsla = hsla(0.15, 0.5, 0.4, 0.5);

    // Integration - purple
    /// Integration category background
    pub const INTEGRATION_BG: Hsla = hsla(0.78, 0.5, 0.3, 0.4);
    /// Integration category text
    pub const INTEGRATION_TEXT: Hsla = hsla(0.78, 0.7, 0.7, 1.0);
    /// Integration category border
    pub const INTEGRATION_BORDER: Hsla = hsla(0.78, 0.5, 0.4, 0.5);

    // Unknown/default - gray
    /// Unknown category background
    pub const UNKNOWN_BG: Hsla = hsla(0.0, 0.0, 0.3, 0.4);
    /// Unknown category text
    pub const UNKNOWN_TEXT: Hsla = hsla(0.0, 0.0, 0.7, 1.0);
    /// Unknown category border
    pub const UNKNOWN_BORDER: Hsla = hsla(0.0, 0.0, 0.4, 0.5);
}

/// HUD-specific colors (pins, units, connections)
pub mod hud {
    use super::*;

    // Pin states
    /// Empty pin color
    pub const PIN_EMPTY: Hsla = hsla(0.0, 0.0, 0.3, 1.0);
    /// Valid pin color (green)
    pub const PIN_VALID: Hsla = hsla(0.33, 0.8, 0.5, 1.0);
    /// Invalid pin color (red)
    pub const PIN_INVALID: Hsla = hsla(0.0, 0.8, 0.5, 1.0);
    /// Constant pin color (cyan)
    pub const PIN_CONSTANT: Hsla = hsla(0.58, 0.8, 0.6, 1.0);

    // Connection states
    /// Active connection (white)
    pub const CONNECTION_ACTIVE: Hsla = hsla(0.0, 0.0, 1.0, 0.8);
    /// Inactive connection (gray)
    pub const CONNECTION_INACTIVE: Hsla = hsla(0.0, 0.0, 0.5, 0.5);
    /// Selected connection (cyan)
    pub const CONNECTION_SELECTED: Hsla = hsla(0.58, 0.8, 0.6, 1.0);

    // Unit states
    /// Playing unit background
    pub const UNIT_PLAYING_BG: Hsla = hsla(0.0, 0.0, 0.15, 0.95);
    /// Paused unit background
    pub const UNIT_PAUSED_BG: Hsla = hsla(0.0, 0.0, 0.1, 0.95);
    /// Error unit background
    pub const UNIT_ERROR_BG: Hsla = hsla(0.0, 0.5, 0.15, 0.95);

    // Graph view
    /// Graph background
    pub const GRAPH_BG: Hsla = hsla(0.0, 0.0, 0.05, 1.0);
    /// Grid lines
    pub const GRID: Hsla = hsla(0.0, 0.0, 0.1, 1.0);
    /// Rubber band selection fill
    pub const RUBBER_BAND_FILL: Hsla = hsla(0.6, 0.8, 0.5, 0.15);
    /// Rubber band selection stroke
    pub const RUBBER_BAND_STROKE: Hsla = hsla(0.6, 0.8, 0.5, 0.8);
}

/// Syntax highlighting colors
pub mod syntax {
    use super::*;

    /// Keywords (if, else, fn, etc.)
    pub const KEYWORD: Hsla = hsla(0.83, 0.6, 0.7, 1.0);
    /// Strings
    pub const STRING: Hsla = hsla(0.38, 0.6, 0.6, 1.0);
    /// Numbers
    pub const NUMBER: Hsla = hsla(0.08, 0.7, 0.7, 1.0);
    /// Comments
    pub const COMMENT: Hsla = hsla(0.0, 0.0, 0.5, 1.0);
    /// Functions
    pub const FUNCTION: Hsla = hsla(0.58, 0.6, 0.7, 1.0);
    /// Types
    pub const TYPE: Hsla = hsla(0.15, 0.6, 0.7, 1.0);
}
