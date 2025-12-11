//! Centralized theme colors for Commander UI
//!
//! All color definitions should go here. Never use inline hsla()/rgb() in components.
//!
//! # Usage
//!
//! ```rust
//! use theme::{bg, text, border, FONT_FAMILY};
//!
//! div()
//!     .bg(bg::SURFACE)
//!     .text_color(text::PRIMARY)
//!     .border_color(border::DEFAULT)
//!     .font_family(FONT_FAMILY)
//! ```

use gpui::Hsla;

/// The standard font family for the entire app
pub const FONT_FAMILY: &str = "Berkeley Mono";

/// Helper to create const Hsla values
const fn c(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla { h, s, l, a }
}

/// Background colors - Dark theme palette
pub mod bg {
    use super::*;

    /// Pure black - main app background (#0A0A0A)
    pub const APP: Hsla = c(0.0, 0.0, 0.04, 1.0);

    /// Near black - surface background (#0A0A0A)
    pub const SURFACE: Hsla = c(0.0, 0.0, 0.04, 1.0);

    /// Slightly elevated surface (#1A1A1A)
    pub const ELEVATED: Hsla = c(0.0, 0.0, 0.10, 1.0);

    /// Card/panel background (#2A2A2A with alpha)
    pub const CARD: Hsla = c(0.0, 0.0, 0.165, 0.6);

    /// Hover state background (#2A2A2A99)
    pub const HOVER: Hsla = c(0.0, 0.0, 0.165, 0.6);

    /// Selected item background (white 5% opacity)
    pub const SELECTED: Hsla = c(0.0, 0.0, 1.0, 0.05);

    /// Darker surface for contrast
    pub const DARK: Hsla = c(0.0, 0.0, 0.02, 1.0);

    /// Panel background (semi-transparent dark)
    pub const PANEL: Hsla = c(0.0, 0.0, 0.04, 0.95);

    /// Row background (very subtle)
    pub const ROW: Hsla = c(0.0, 0.0, 0.06, 0.6);

    /// Subtle row background
    pub const ROW_SUBTLE: Hsla = c(0.0, 0.0, 0.06, 0.3);

    /// Header background
    pub const HEADER: Hsla = c(0.0, 0.0, 0.08, 0.8);

    /// Header hover
    pub const HEADER_HOVER: Hsla = c(0.0, 0.0, 0.10, 0.8);

    /// Sidebar background (#0A0A0A)
    pub const SIDEBAR: Hsla = c(0.0, 0.0, 0.04, 1.0);

    /// Sidebar header background
    pub const SIDEBAR_HEADER: Hsla = c(0.0, 0.0, 0.06, 1.0);

    /// Code block background
    pub const CODE: Hsla = c(0.0, 0.0, 0.10, 1.0);
}

/// Border colors
pub mod border {
    use super::*;

    /// Default border (#ffffff1a - white 10% alpha)
    pub const DEFAULT: Hsla = c(0.0, 0.0, 1.0, 0.10);

    /// Subtle/faint border
    pub const SUBTLE: Hsla = c(0.0, 0.0, 1.0, 0.06);

    /// Strong/prominent border (#2A2A2A)
    pub const STRONG: Hsla = c(0.0, 0.0, 0.165, 1.0);

    /// Selected item border (white 30% opacity)
    pub const SELECTED: Hsla = c(0.0, 0.0, 1.0, 0.30);

    /// Focus ring color (cyan)
    pub const FOCUS: Hsla = c(0.54, 0.43, 0.67, 0.8);
}

/// Text colors
pub mod text {
    use super::*;

    /// Primary text - main content (#D8DEE9)
    pub const PRIMARY: Hsla = c(0.58, 0.20, 0.88, 1.0);

    /// Bright text - values, data (#FFFFFF)
    pub const BRIGHT: Hsla = c(0.0, 0.0, 1.0, 1.0);

    /// Secondary text - less emphasis (#CCCCCC)
    pub const SECONDARY: Hsla = c(0.0, 0.0, 0.80, 1.0);

    /// Muted text - labels, hints (#CCCCCC99)
    pub const MUTED: Hsla = c(0.0, 0.0, 0.80, 0.60);

    /// Disabled text (#505050)
    pub const DISABLED: Hsla = c(0.0, 0.0, 0.31, 1.0);

    /// Very dim text - separators, dots
    pub const DIM: Hsla = c(0.0, 0.0, 0.25, 1.0);

    /// Placeholder text (#FFFFFF99)
    pub const PLACEHOLDER: Hsla = c(0.0, 0.0, 1.0, 0.60);

    /// Link text color (cyan #88C0D0)
    pub const LINK: Hsla = c(0.54, 0.43, 0.67, 1.0);
}

/// Semantic/status colors
pub mod status {
    use super::*;

    // Success/valid - green (#A3BE8C)
    /// Success text/icon color
    pub const SUCCESS: Hsla = c(0.26, 0.27, 0.65, 1.0);
    /// Success background
    pub const SUCCESS_BG: Hsla = c(0.26, 0.27, 0.30, 0.4);
    /// Success border
    pub const SUCCESS_BORDER: Hsla = c(0.26, 0.27, 0.45, 0.5);

    // Error/invalid - red (#BF616A)
    /// Error text/icon color
    pub const ERROR: Hsla = c(0.99, 0.42, 0.56, 1.0);
    /// Error background
    pub const ERROR_BG: Hsla = c(0.99, 0.42, 0.25, 0.4);
    /// Error border
    pub const ERROR_BORDER: Hsla = c(0.99, 0.42, 0.40, 0.5);

    // Warning - yellow (#EBCB8B)
    /// Warning text/icon color
    pub const WARNING: Hsla = c(0.12, 0.72, 0.73, 1.0);
    /// Warning background
    pub const WARNING_BG: Hsla = c(0.12, 0.50, 0.30, 0.4);
    /// Warning border
    pub const WARNING_BORDER: Hsla = c(0.12, 0.50, 0.45, 0.5);

    // Info - cyan (#88C0D0)
    /// Info text/icon color
    pub const INFO: Hsla = c(0.54, 0.43, 0.67, 1.0);
    /// Info background
    pub const INFO_BG: Hsla = c(0.54, 0.43, 0.30, 0.4);
    /// Info border
    pub const INFO_BORDER: Hsla = c(0.54, 0.43, 0.45, 0.5);

    // Running/in-progress - yellow
    /// Running state text
    pub const RUNNING: Hsla = c(0.12, 0.72, 0.73, 1.0);
    /// Running state background
    pub const RUNNING_BG: Hsla = c(0.12, 0.50, 0.30, 0.4);

    // Pending - gray
    /// Pending state text
    pub const PENDING: Hsla = c(0.0, 0.0, 0.50, 1.0);
    /// Pending state background
    pub const PENDING_BG: Hsla = c(0.0, 0.0, 0.20, 0.4);
}

/// Accent/brand colors
pub mod accent {
    use super::*;

    /// Primary accent - cyan (#88C0D0 / #83D6C5)
    pub const PRIMARY: Hsla = c(0.54, 0.43, 0.67, 1.0);

    /// Secondary accent - purple (#B48EAD / #AA9BF5)
    pub const SECONDARY: Hsla = c(0.69, 0.60, 0.70, 1.0);

    /// Tertiary accent - green (#A3BE8C)
    pub const TERTIARY: Hsla = c(0.26, 0.27, 0.65, 1.0);

    /// Blue accent (#81A1C1 / #87C3FF)
    pub const BLUE: Hsla = c(0.58, 0.50, 0.70, 1.0);

    /// Orange accent (#EFB080)
    pub const ORANGE: Hsla = c(0.08, 0.75, 0.72, 1.0);

    /// Muted cyan for backgrounds
    pub const PRIMARY_MUTED: Hsla = c(0.54, 0.43, 0.30, 0.4);

    /// Muted purple for backgrounds
    pub const SECONDARY_MUTED: Hsla = c(0.69, 0.40, 0.30, 0.4);
}

/// Source badges (ATIF step sources)
pub mod source {
    use super::*;

    // User - cyan (#88C0D0)
    /// User source badge background
    pub const USER_BG: Hsla = c(0.54, 0.43, 0.30, 0.4);
    /// User source badge text
    pub const USER_TEXT: Hsla = c(0.54, 0.43, 0.75, 1.0);
    /// User source badge border
    pub const USER_BORDER: Hsla = c(0.54, 0.43, 0.50, 0.5);

    // Agent - green (#A3BE8C)
    /// Agent source badge background
    pub const AGENT_BG: Hsla = c(0.26, 0.27, 0.30, 0.4);
    /// Agent source badge text
    pub const AGENT_TEXT: Hsla = c(0.26, 0.27, 0.75, 1.0);
    /// Agent source badge border
    pub const AGENT_BORDER: Hsla = c(0.26, 0.27, 0.50, 0.5);

    // System - gray
    /// System source badge background
    pub const SYSTEM_BG: Hsla = c(0.0, 0.0, 0.20, 0.4);
    /// System source badge text
    pub const SYSTEM_TEXT: Hsla = c(0.0, 0.0, 0.70, 1.0);
    /// System source badge border
    pub const SYSTEM_BORDER: Hsla = c(0.0, 0.0, 0.40, 0.5);
}

/// Test category badges
pub mod category {
    use super::*;

    // AntiCheat - red (#BF616A)
    /// AntiCheat category background
    pub const ANTI_CHEAT_BG: Hsla = c(0.99, 0.42, 0.30, 0.4);
    /// AntiCheat category text
    pub const ANTI_CHEAT_TEXT: Hsla = c(0.99, 0.42, 0.70, 1.0);
    /// AntiCheat category border
    pub const ANTI_CHEAT_BORDER: Hsla = c(0.99, 0.42, 0.45, 0.5);

    // Existence - blue (#81A1C1)
    /// Existence category background
    pub const EXISTENCE_BG: Hsla = c(0.58, 0.30, 0.30, 0.4);
    /// Existence category text
    pub const EXISTENCE_TEXT: Hsla = c(0.58, 0.50, 0.75, 1.0);
    /// Existence category border
    pub const EXISTENCE_BORDER: Hsla = c(0.58, 0.40, 0.50, 0.5);

    // Correctness - green (#A3BE8C)
    /// Correctness category background
    pub const CORRECTNESS_BG: Hsla = c(0.26, 0.27, 0.30, 0.4);
    /// Correctness category text
    pub const CORRECTNESS_TEXT: Hsla = c(0.26, 0.27, 0.75, 1.0);
    /// Correctness category border
    pub const CORRECTNESS_BORDER: Hsla = c(0.26, 0.27, 0.50, 0.5);

    // Boundary - yellow (#EBCB8B)
    /// Boundary category background
    pub const BOUNDARY_BG: Hsla = c(0.12, 0.50, 0.30, 0.4);
    /// Boundary category text
    pub const BOUNDARY_TEXT: Hsla = c(0.12, 0.72, 0.75, 1.0);
    /// Boundary category border
    pub const BOUNDARY_BORDER: Hsla = c(0.12, 0.50, 0.50, 0.5);

    // Integration - purple (#B48EAD)
    /// Integration category background
    pub const INTEGRATION_BG: Hsla = c(0.89, 0.28, 0.30, 0.4);
    /// Integration category text
    pub const INTEGRATION_TEXT: Hsla = c(0.89, 0.28, 0.70, 1.0);
    /// Integration category border
    pub const INTEGRATION_BORDER: Hsla = c(0.89, 0.28, 0.50, 0.5);

    // Unknown/default - gray
    /// Unknown category background
    pub const UNKNOWN_BG: Hsla = c(0.0, 0.0, 0.25, 0.4);
    /// Unknown category text
    pub const UNKNOWN_TEXT: Hsla = c(0.0, 0.0, 0.70, 1.0);
    /// Unknown category border
    pub const UNKNOWN_BORDER: Hsla = c(0.0, 0.0, 0.40, 0.5);
}

/// HUD-specific colors (pins, units, connections)
pub mod hud {
    use super::*;

    // Pin states
    /// Empty pin color
    pub const PIN_EMPTY: Hsla = c(0.0, 0.0, 0.31, 1.0);
    /// Valid pin color (green #A3BE8C)
    pub const PIN_VALID: Hsla = c(0.26, 0.27, 0.65, 1.0);
    /// Invalid pin color (red #BF616A)
    pub const PIN_INVALID: Hsla = c(0.99, 0.42, 0.56, 1.0);
    /// Constant pin color (cyan #88C0D0)
    pub const PIN_CONSTANT: Hsla = c(0.54, 0.43, 0.67, 1.0);

    // Connection states
    /// Active connection (white)
    pub const CONNECTION_ACTIVE: Hsla = c(0.0, 0.0, 1.0, 0.8);
    /// Inactive connection (gray)
    pub const CONNECTION_INACTIVE: Hsla = c(0.0, 0.0, 0.50, 0.5);
    /// Selected connection (cyan)
    pub const CONNECTION_SELECTED: Hsla = c(0.54, 0.43, 0.67, 1.0);

    // Unit states
    /// Playing unit background
    pub const UNIT_PLAYING_BG: Hsla = c(0.0, 0.0, 0.10, 0.95);
    /// Paused unit background
    pub const UNIT_PAUSED_BG: Hsla = c(0.0, 0.0, 0.06, 0.95);
    /// Error unit background (red tint)
    pub const UNIT_ERROR_BG: Hsla = c(0.99, 0.30, 0.15, 0.95);

    // Graph view
    /// Graph background (#0A0A0A)
    pub const GRAPH_BG: Hsla = c(0.0, 0.0, 0.04, 1.0);
    /// Grid lines
    pub const GRID: Hsla = c(0.0, 0.0, 0.10, 1.0);
    /// Rubber band selection fill (cyan)
    pub const RUBBER_BAND_FILL: Hsla = c(0.54, 0.43, 0.50, 0.15);
    /// Rubber band selection stroke
    pub const RUBBER_BAND_STROKE: Hsla = c(0.54, 0.43, 0.67, 0.8);

    // APM widget colors
    /// APM widget background (#141017)
    pub const APM_WIDGET_BG: Hsla = c(0.75, 0.17, 0.08, 0.95);
    /// APM widget border
    pub const APM_WIDGET_BORDER: Hsla = c(0.11, 0.91, 0.60, 0.25);

    // APM velocity levels (based on actions per minute)
    /// APM baseline/idle (0-5 APM) - gray #6b7280
    pub const APM_BASELINE: Hsla = c(0.61, 0.07, 0.46, 1.0);
    /// APM active (5-15 APM) - blue #3b82f6
    pub const APM_ACTIVE: Hsla = c(0.61, 0.91, 0.60, 1.0);
    /// APM high velocity (15-30 APM) - green #22c55e
    pub const APM_HIGH: Hsla = c(0.40, 0.70, 0.45, 1.0);
    /// APM elite performance (30+ APM) - amber/gold #f59e0b
    pub const APM_ELITE: Hsla = c(0.11, 0.91, 0.50, 1.0);

    // Task status colors
    /// Pending status - zinc #71717a
    pub const STATUS_PENDING: Hsla = c(0.67, 0.04, 0.47, 1.0);
    /// Running status - blue #3b82f6
    pub const STATUS_RUNNING: Hsla = c(0.61, 0.91, 0.60, 1.0);
    /// Passed status - emerald #34d399
    pub const STATUS_PASSED: Hsla = c(0.44, 0.62, 0.52, 1.0);
    /// Failed status - red #f87171
    pub const STATUS_FAILED: Hsla = c(0.0, 0.90, 0.71, 1.0);
    /// Timeout status - amber #fbbf24
    pub const STATUS_TIMEOUT: Hsla = c(0.11, 0.95, 0.56, 1.0);
    /// Error status - orange #fb923c
    pub const STATUS_ERROR: Hsla = c(0.07, 0.96, 0.61, 1.0);

    // Difficulty badges
    /// Easy difficulty background - emerald-900/40
    pub const DIFFICULTY_EASY_BG: Hsla = c(0.44, 0.50, 0.15, 0.4);
    /// Easy difficulty text - emerald-300
    pub const DIFFICULTY_EASY_TEXT: Hsla = c(0.44, 0.62, 0.58, 1.0);
    /// Medium difficulty background - amber-900/40
    pub const DIFFICULTY_MEDIUM_BG: Hsla = c(0.11, 0.50, 0.15, 0.4);
    /// Medium difficulty text - amber-300
    pub const DIFFICULTY_MEDIUM_TEXT: Hsla = c(0.11, 0.80, 0.65, 1.0);
    /// Hard difficulty background - red-900/40
    pub const DIFFICULTY_HARD_BG: Hsla = c(0.0, 0.50, 0.15, 0.4);
    /// Hard difficulty text - red-300
    pub const DIFFICULTY_HARD_TEXT: Hsla = c(0.0, 0.80, 0.69, 1.0);
    /// Unknown difficulty background - zinc-800/40
    pub const DIFFICULTY_UNKNOWN_BG: Hsla = c(0.0, 0.0, 0.20, 0.4);
    /// Unknown difficulty text - zinc-300
    pub const DIFFICULTY_UNKNOWN_TEXT: Hsla = c(0.0, 0.0, 0.80, 1.0);

    // Panel/container colors
    /// Panel background - zinc-950/95
    pub const PANEL_BG: Hsla = c(0.0, 0.0, 0.04, 0.95);
    /// Panel border - zinc-800/60
    pub const PANEL_BORDER: Hsla = c(0.0, 0.0, 0.20, 0.6);
    /// Header background - zinc-900/80
    pub const HEADER_BG: Hsla = c(0.0, 0.0, 0.08, 0.8);
    /// Divider line - zinc-800/40
    pub const DIVIDER: Hsla = c(0.0, 0.0, 0.20, 0.4);

    // Button colors
    /// Button border - zinc-700/50
    pub const BUTTON_BORDER: Hsla = c(0.0, 0.0, 0.30, 0.5);
    /// Button border on hover - zinc-600/60
    pub const BUTTON_BORDER_HOVER: Hsla = c(0.0, 0.0, 0.40, 0.6);

    // Row colors
    /// Row hover background - zinc-900/40
    pub const ROW_HOVER: Hsla = c(0.0, 0.0, 0.08, 0.4);
    /// Row selected background - zinc-800/60
    pub const ROW_SELECTED: Hsla = c(0.0, 0.0, 0.20, 0.6);
}

/// Syntax highlighting colors
pub mod syntax {
    use super::*;

    /// Keywords - cyan (#83D6C5)
    pub const KEYWORD: Hsla = c(0.46, 0.47, 0.67, 1.0);
    /// Strings - pink (#E394DC)
    pub const STRING: Hsla = c(0.88, 0.60, 0.73, 1.0);
    /// Numbers - yellow (#EBC88D)
    pub const NUMBER: Hsla = c(0.10, 0.70, 0.74, 1.0);
    /// Comments - gray (#FFFFFF5C)
    pub const COMMENT: Hsla = c(0.0, 0.0, 1.0, 0.36);
    /// Functions - orange (#EFB080)
    pub const FUNCTION: Hsla = c(0.08, 0.75, 0.72, 1.0);
    /// Types - blue (#87C3FF)
    pub const TYPE: Hsla = c(0.58, 1.0, 0.77, 1.0);
    /// Variables - light gray (#D6D6DD)
    pub const VARIABLE: Hsla = c(0.67, 0.05, 0.85, 1.0);
    /// Properties - purple (#AA9BF5)
    pub const PROPERTY: Hsla = c(0.69, 0.82, 0.78, 1.0);
}

/// Text input colors (white-based for dark backgrounds)
pub mod input {
    use super::*;

    /// Placeholder text color (#FFFFFF99)
    pub const PLACEHOLDER: Hsla = c(0.0, 0.0, 1.0, 0.60);
    /// Input text color
    pub const TEXT: Hsla = c(0.0, 0.0, 1.0, 0.87);
    /// Cursor color
    pub const CURSOR: Hsla = c(0.0, 0.0, 1.0, 1.0);
    /// Selection background (#40404099)
    pub const SELECTION: Hsla = c(0.0, 0.0, 0.25, 0.60);
    /// Input background (#2A2A2A55)
    pub const BG: Hsla = c(0.0, 0.0, 0.165, 0.33);
    /// Input border (#2A2A2A)
    pub const BORDER: Hsla = c(0.0, 0.0, 0.165, 1.0);
}

/// Trust tier colors (marketplace progression system)
pub mod trust {
    use super::*;

    // Bronze tier - warm brown/copper
    /// Bronze tier primary color
    pub const BRONZE: Hsla = c(0.08, 0.60, 0.40, 1.0);
    /// Bronze tier background
    pub const BRONZE_BG: Hsla = c(0.08, 0.40, 0.20, 0.4);
    /// Bronze tier border
    pub const BRONZE_BORDER: Hsla = c(0.08, 0.50, 0.35, 0.5);

    // Silver tier - cool gray
    /// Silver tier primary color
    pub const SILVER: Hsla = c(0.0, 0.0, 0.75, 1.0);
    /// Silver tier background
    pub const SILVER_BG: Hsla = c(0.0, 0.0, 0.40, 0.4);
    /// Silver tier border
    pub const SILVER_BORDER: Hsla = c(0.0, 0.0, 0.60, 0.5);

    // Gold tier - warm yellow/gold
    /// Gold tier primary color
    pub const GOLD: Hsla = c(0.14, 1.0, 0.50, 1.0);
    /// Gold tier background
    pub const GOLD_BG: Hsla = c(0.14, 0.70, 0.30, 0.4);
    /// Gold tier border
    pub const GOLD_BORDER: Hsla = c(0.14, 0.80, 0.45, 0.5);

    // Diamond tier - cyan/blue (premium)
    /// Diamond tier primary color
    pub const DIAMOND: Hsla = c(0.54, 0.80, 0.70, 1.0);
    /// Diamond tier background
    pub const DIAMOND_BG: Hsla = c(0.54, 0.60, 0.30, 0.4);
    /// Diamond tier border
    pub const DIAMOND_BORDER: Hsla = c(0.54, 0.70, 0.50, 0.5);
}

/// UI component colors (shadcn-style components - zinc grayscale theme)
/// Based on Tailwind zinc palette:
/// zinc-50: #fafafa, zinc-100: #f4f4f5, zinc-200: #e4e4e7, zinc-300: #d4d4d8
/// zinc-400: #a1a1aa, zinc-500: #71717a, zinc-600: #52525b, zinc-700: #3f3f46
/// zinc-800: #27272a, zinc-900: #18181b, zinc-950: #09090b
pub mod ui {
    use super::*;

    /// Button colors - grayscale zinc theme
    pub mod button {
        use super::*;

        // Default variant - white/light (zinc-50/100) - primary action
        pub const DEFAULT_BG: Hsla = c(0.0, 0.0, 0.98, 1.0);        // zinc-50 #fafafa
        pub const DEFAULT_TEXT: Hsla = c(0.0, 0.0, 0.09, 1.0);      // zinc-900 #18181b
        pub const DEFAULT_HOVER_BG: Hsla = c(0.0, 0.0, 0.90, 1.0);  // zinc-200-ish

        // Destructive variant - red (keep for semantic meaning)
        pub const DESTRUCTIVE_BG: Hsla = c(0.99, 0.42, 0.56, 1.0);
        pub const DESTRUCTIVE_TEXT: Hsla = c(0.0, 0.0, 1.0, 1.0);
        pub const DESTRUCTIVE_HOVER_BG: Hsla = c(0.99, 0.42, 0.48, 1.0);

        // Outline variant - transparent with zinc border
        pub const OUTLINE_BG: Hsla = c(0.0, 0.0, 0.0, 0.0);
        pub const OUTLINE_TEXT: Hsla = c(0.0, 0.0, 0.90, 1.0);      // zinc-100
        pub const OUTLINE_BORDER: Hsla = c(0.0, 0.0, 0.27, 1.0);    // zinc-800 #27272a
        pub const OUTLINE_HOVER_BG: Hsla = c(0.0, 0.0, 0.27, 1.0);  // zinc-800

        // Secondary variant - zinc-800
        pub const SECONDARY_BG: Hsla = c(0.0, 0.0, 0.15, 1.0);      // zinc-800-ish
        pub const SECONDARY_TEXT: Hsla = c(0.0, 0.0, 0.90, 1.0);    // zinc-100
        pub const SECONDARY_HOVER_BG: Hsla = c(0.0, 0.0, 0.20, 1.0);

        // Ghost variant - transparent, shows on hover
        pub const GHOST_BG: Hsla = c(0.0, 0.0, 0.0, 0.0);
        pub const GHOST_TEXT: Hsla = c(0.0, 0.0, 0.90, 1.0);        // zinc-100
        pub const GHOST_HOVER_BG: Hsla = c(0.0, 0.0, 0.27, 1.0);    // zinc-800

        // Link variant - just underlined text, zinc-400
        pub const LINK_TEXT: Hsla = c(0.0, 0.0, 0.63, 1.0);         // zinc-400 #a1a1aa
        pub const LINK_HOVER_TEXT: Hsla = c(0.0, 0.0, 0.90, 1.0);   // zinc-100
    }

    /// Checkbox colors - grayscale
    pub mod checkbox {
        use super::*;

        pub const UNCHECKED_BG: Hsla = c(0.0, 0.0, 0.0, 0.0);
        pub const UNCHECKED_BORDER: Hsla = c(0.0, 0.0, 0.44, 1.0);  // zinc-500
        pub const CHECKED_BG: Hsla = c(0.0, 0.0, 0.98, 1.0);        // zinc-50 (white)
        pub const CHECKED_BORDER: Hsla = c(0.0, 0.0, 0.98, 1.0);    // zinc-50
        pub const CHECK_ICON: Hsla = c(0.0, 0.0, 0.09, 1.0);        // zinc-900 (black)
    }

    /// Switch/toggle colors - grayscale
    pub mod switch {
        use super::*;

        pub const TRACK_OFF: Hsla = c(0.0, 0.0, 0.25, 1.0);         // zinc-800
        pub const TRACK_ON: Hsla = c(0.0, 0.0, 0.98, 1.0);          // zinc-50 (white)
        pub const THUMB: Hsla = c(0.0, 0.0, 1.0, 1.0);              // pure white
        pub const THUMB_ON: Hsla = c(0.0, 0.0, 0.09, 1.0);          // zinc-900 when on
    }

    /// Progress bar colors - grayscale
    pub mod progress {
        use super::*;

        pub const TRACK_BG: Hsla = c(0.0, 0.0, 0.15, 1.0);          // zinc-800-ish
        pub const INDICATOR: Hsla = c(0.0, 0.0, 0.98, 1.0);         // zinc-50 (white)
    }

    /// Skeleton loading colors - grayscale
    pub mod skeleton {
        use super::*;

        pub const BG: Hsla = c(0.0, 0.0, 0.15, 1.0);                // zinc-800
        pub const SHIMMER: Hsla = c(0.0, 0.0, 0.25, 1.0);           // zinc-700
    }

    /// Separator colors - grayscale
    pub mod separator {
        use super::*;

        pub const DEFAULT: Hsla = c(0.0, 0.0, 0.17, 1.0);           // zinc-800 #27272a
    }

    /// Label colors - grayscale
    pub mod label {
        use super::*;

        pub const TEXT: Hsla = c(0.0, 0.0, 0.90, 1.0);              // zinc-100
        pub const DISABLED: Hsla = c(0.0, 0.0, 0.44, 1.0);          // zinc-500
    }

    /// Kbd (keyboard) colors - grayscale
    pub mod kbd {
        use super::*;

        pub const BG: Hsla = c(0.0, 0.0, 0.15, 1.0);                // zinc-800
        pub const BORDER: Hsla = c(0.0, 0.0, 0.25, 1.0);            // zinc-700
        pub const TEXT: Hsla = c(0.0, 0.0, 0.83, 1.0);              // zinc-200
    }
}

// ============================================================================
// Zed-compatible theme API (minimal implementation for compatibility)
// ============================================================================

use std::sync::Arc;
use gpui::{App, Global};

/// Theme struct for Zed compatibility
#[derive(Clone)]
pub struct Theme {
    pub name: String,
}

impl Theme {
    /// Get theme colors
    pub fn colors(&self) -> ThemeColors {
        ThemeColors::default()
    }

    /// Get syntax colors
    pub fn syntax(&self) -> &SyntaxTheme {
        &SYNTAX_THEME
    }

    /// Get status colors
    pub fn status(&self) -> StatusColors {
        StatusColors
    }

    /// Get players (stub)
    pub fn players(&self) -> Players {
        Players
    }
}

/// Theme colors accessor
#[derive(Clone, Copy)]
pub struct ThemeColors {
    // Terminal ANSI colors
    pub terminal_ansi_black: Hsla,
    pub terminal_ansi_red: Hsla,
    pub terminal_ansi_green: Hsla,
    pub terminal_ansi_yellow: Hsla,
    pub terminal_ansi_blue: Hsla,
    pub terminal_ansi_magenta: Hsla,
    pub terminal_ansi_cyan: Hsla,
    pub terminal_ansi_white: Hsla,
    pub terminal_ansi_bright_black: Hsla,
    pub terminal_ansi_bright_red: Hsla,
    pub terminal_ansi_bright_green: Hsla,
    pub terminal_ansi_bright_yellow: Hsla,
    pub terminal_ansi_bright_blue: Hsla,
    pub terminal_ansi_bright_magenta: Hsla,
    pub terminal_ansi_bright_cyan: Hsla,
    pub terminal_ansi_bright_white: Hsla,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            terminal_ansi_black: c(0.0, 0.0, 0.0, 1.0),
            terminal_ansi_red: c(0.0, 0.70, 0.50, 1.0),
            terminal_ansi_green: c(0.33, 0.70, 0.50, 1.0),
            terminal_ansi_yellow: c(0.12, 0.70, 0.60, 1.0),
            terminal_ansi_blue: c(0.58, 0.70, 0.50, 1.0),
            terminal_ansi_magenta: c(0.83, 0.70, 0.50, 1.0),
            terminal_ansi_cyan: c(0.50, 0.70, 0.50, 1.0),
            terminal_ansi_white: c(0.0, 0.0, 0.80, 1.0),
            terminal_ansi_bright_black: c(0.0, 0.0, 0.30, 1.0),
            terminal_ansi_bright_red: c(0.0, 0.80, 0.65, 1.0),
            terminal_ansi_bright_green: c(0.33, 0.80, 0.65, 1.0),
            terminal_ansi_bright_yellow: c(0.12, 0.80, 0.70, 1.0),
            terminal_ansi_bright_blue: c(0.58, 0.80, 0.65, 1.0),
            terminal_ansi_bright_magenta: c(0.83, 0.80, 0.65, 1.0),
            terminal_ansi_bright_cyan: c(0.50, 0.80, 0.65, 1.0),
            terminal_ansi_bright_white: c(0.0, 0.0, 1.0, 1.0),
        }
    }
}

impl ThemeColors {
    pub fn surface_background(&self) -> Hsla { bg::SURFACE }
    pub fn background(&self) -> Hsla { bg::APP }
    pub fn text(&self) -> Hsla { text::PRIMARY }
    pub fn text_muted(&self) -> Hsla { text::MUTED }
    pub fn border(&self) -> Hsla { border::DEFAULT }
    pub fn border_variant(&self) -> Hsla { border::SUBTLE }
    pub fn element_background(&self) -> Hsla { bg::ELEVATED }
    pub fn element_hover(&self) -> Hsla { bg::HOVER }
    pub fn element_selected(&self) -> Hsla { bg::SELECTED }
    pub fn ghost_element_hover(&self) -> Hsla { bg::HOVER }
    pub fn editor_background(&self) -> Hsla { bg::SURFACE }
    pub fn editor_foreground(&self) -> Hsla { text::PRIMARY }
    pub fn editor_line_number(&self) -> Hsla { text::DIM }
    pub fn editor_active_line_number(&self) -> Hsla { text::SECONDARY }
    pub fn terminal_background(&self) -> Hsla { bg::SURFACE }
    pub fn terminal_foreground(&self) -> Hsla { text::PRIMARY }
    pub fn link_text_hover(&self) -> Hsla { text::LINK }
}

/// Status colors accessor
#[derive(Clone, Copy)]
pub struct StatusColors;

impl StatusColors {
    pub fn created(&self) -> Hsla { status::SUCCESS }
    pub fn created_background(&self) -> Hsla { status::SUCCESS_BG }
    pub fn deleted(&self) -> Hsla { status::ERROR }
    pub fn deleted_background(&self) -> Hsla { status::ERROR_BG }
    pub fn modified(&self) -> Hsla { status::WARNING }
    pub fn modified_background(&self) -> Hsla { status::WARNING_BG }
    pub fn info(&self) -> Hsla { status::INFO }
    pub fn info_background(&self) -> Hsla { status::INFO_BG }
    pub fn success(&self) -> Hsla { status::SUCCESS }
    pub fn success_background(&self) -> Hsla { status::SUCCESS_BG }
    pub fn warning(&self) -> Hsla { status::WARNING }
    pub fn warning_background(&self) -> Hsla { status::WARNING_BG }
    pub fn error(&self) -> Hsla { status::ERROR }
    pub fn error_background(&self) -> Hsla { status::ERROR_BG }
}

/// Syntax theme
pub struct SyntaxTheme;

static SYNTAX_THEME: SyntaxTheme = SyntaxTheme;

impl SyntaxTheme {
    pub fn highlight_style(&self, name: &str) -> Option<HighlightStyle> {
        let color = match name {
            "keyword" => syntax::KEYWORD,
            "string" => syntax::STRING,
            "number" => syntax::NUMBER,
            "comment" => syntax::COMMENT,
            "function" => syntax::FUNCTION,
            "type" => syntax::TYPE,
            "variable" => syntax::VARIABLE,
            "property" => syntax::PROPERTY,
            _ => text::PRIMARY,
        };
        Some(HighlightStyle { color: Some(color), ..Default::default() })
    }
}

/// Highlight style for syntax
#[derive(Clone, Default)]
pub struct HighlightStyle {
    pub color: Option<Hsla>,
    pub font_weight: Option<gpui::FontWeight>,
    pub font_style: Option<gpui::FontStyle>,
}

/// Players (stub for collaboration)
pub struct Players;

impl Players {
    pub fn local(&self) -> PlayerColors {
        PlayerColors { cursor: accent::PRIMARY, selection: accent::PRIMARY_MUTED }
    }
}

pub struct PlayerColors {
    pub cursor: Hsla,
    pub selection: Hsla,
}

/// Global theme state
pub struct ActiveTheme(pub Arc<Theme>);

impl Global for ActiveTheme {}

/// Stub types for Zed compatibility
pub type GlobalTheme = ActiveTheme;
pub type ThemeRegistry = ();
pub type IconTheme = ();
pub type FontFamilyName = String;

/// Initialize the theme system
pub fn init(cx: &mut App) {
    let theme = Arc::new(Theme {
        name: "OpenAgents Dark".to_string(),
    });
    cx.set_global(ActiveTheme(theme));
}

/// Get the active theme
pub fn active_theme(cx: &App) -> Arc<Theme> {
    cx.try_global::<ActiveTheme>()
        .map(|t| t.0.clone())
        .unwrap_or_else(|| Arc::new(Theme { name: "OpenAgents Dark".to_string() }))
}

/// Extension trait for App to get theme
pub trait ActiveThemeExt {
    fn theme(&self) -> Arc<Theme>;
}

impl ActiveThemeExt for App {
    fn theme(&self) -> Arc<Theme> {
        active_theme(self)
    }
}

impl<T> ActiveThemeExt for gpui::Context<'_, T> {
    fn theme(&self) -> Arc<Theme> {
        active_theme(self)
    }
}
