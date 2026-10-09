//! Coder Noir: Coder's neutral accents over Superlogical's "Static Noir"
//! palette. This is the one source of the Noir values: `coder-ui`'s
//! `coder_noir` module re-exports them for native rendering, and [`OVERRIDES`]
//! maps them onto Apps SDK UI roles for the web token stylesheets.
//!
//! Per the adoption plan (decision 5), Noir wins where it defines a role:
//! canvas and surfaces, strokes, the content ladder, the neutral accent and
//! the focus ring. Every other role (intents such as danger, caution and
//! discovery, with their soft, outline and ghost states) keeps Apps SDK UI's
//! dark value.
//!
//! Terminal values (`TERMINAL_*`, [`ANSI`]) are dark in every theme: terminal
//! UIs and terminal panels stay Coder Noir only.

pub const CANVAS: u32 = 0x0a0a0a;
pub const SURFACE_SUBTLE: u32 = 0x101010;
pub const SURFACE_RAISED: u32 = 0x191919;
pub const SURFACE: u32 = 0x0e0e0e;
pub const TERMINAL_BACKGROUND: u32 = 0x0e0e0e;
pub const STROKE_SUBTLE: u32 = 0x2c2c2c;
pub const STROKE: u32 = 0x404040;
pub const SCROLLBAR_HOVER: u32 = 0x4e4e4e;
pub const CONTROL_HOVER: u32 = 0xededed;
pub const CONTROL: u32 = 0xededed;
pub const CONTROL_ON_OVERLAY: u32 = 0x1e1e1e;
pub const CONTROL_PRESSED: u32 = 0xededed;
pub const CONTENT: u32 = 0xededed;
pub const CONTENT_SECONDARY: u32 = 0x818181;
pub const CONTENT_TERTIARY: u32 = 0x656565;
pub const ACCENT: u32 = 0xededed;
pub const ACCENT_SOLID: u32 = 0xededed;
pub const ACCENT_ON_SOLID: u32 = 0x0a0a0a;
pub const ACCENT_CONTAINER: u32 = 0x191919;
pub const ACCENT_ON_CONTAINER: u32 = 0xededed;
pub const ACCENT_BORDER: u32 = 0xededed;
pub const ACCENT_RGB: u32 = 0xededed;
pub const ACCENT_DIM: u32 = 0xededed;
pub const ACCENT_DIM_SUBTLE: u32 = 0xededed;
pub const ACCENT_LINE: u32 = 0xededed;
pub const ACCENT_RING_OUTER: u32 = 0xededed;
pub const SELECTION: u32 = 0xededed;
pub const SELECTION_FOREGROUND: u32 = 0x0a0a0a;
pub const DANGER: u32 = 0xff4d42;
pub const DANGER_SOLID: u32 = 0xff4d42;
pub const DANGER_ON_SOLID: u32 = 0x0a0a0a;
pub const DANGER_CONTAINER: u32 = 0x2b1613;
pub const DANGER_ON_CONTAINER: u32 = 0xff4d42;
pub const DANGER_BORDER: u32 = 0xff4d42;
pub const DANGER_BG: u32 = 0x2b1613;
pub const SUCCESS: u32 = 0x9fd08a;
pub const SUCCESS_SOLID: u32 = 0x9fd08a;
pub const SUCCESS_ON_SOLID: u32 = 0x0a0a0a;
pub const SUCCESS_CONTAINER: u32 = 0x1e241c;
pub const SUCCESS_ON_CONTAINER: u32 = 0x9fd08a;
pub const SUCCESS_BORDER: u32 = 0x9fd08a;
pub const WARNING: u32 = 0xe6c15c;
pub const WARNING_SOLID: u32 = 0xe6c15c;
pub const WARNING_ON_SOLID: u32 = 0x0a0a0a;
pub const WARNING_CONTAINER: u32 = 0x262217;
pub const WARNING_ON_CONTAINER: u32 = 0xe6c15c;
pub const WARNING_BORDER: u32 = 0xe6c15c;
pub const INFO: u32 = 0x7fb2e8;
pub const INFO_SOLID: u32 = 0x7fb2e8;
pub const INFO_ON_SOLID: u32 = 0x0a0a0a;
pub const INFO_CONTAINER: u32 = 0x1a2027;
pub const INFO_ON_CONTAINER: u32 = 0x7fb2e8;
pub const INFO_BORDER: u32 = 0x7fb2e8;
pub const TERMINAL_FOREGROUND: u32 = 0xededed;
pub const TERMINAL_CURSOR: u32 = 0xededed;
pub const TERMINAL_SELECTION: u32 = 0x333333;
pub const TERMINAL_SELECTION_FOREGROUND: u32 = 0xffffff;
pub const TERMINAL_ANSI_0: u32 = 0x1a1a1a;
pub const TERMINAL_ANSI_1: u32 = 0xff4d42;
pub const TERMINAL_ANSI_2: u32 = 0x9fd08a;
pub const TERMINAL_ANSI_3: u32 = 0xe6c15c;
pub const TERMINAL_ANSI_4: u32 = 0x7fb2e8;
pub const TERMINAL_ANSI_5: u32 = 0xd093d0;
pub const TERMINAL_ANSI_6: u32 = 0x74cfd1;
pub const TERMINAL_ANSI_7: u32 = 0xc9c9c9;
pub const TERMINAL_ANSI_8: u32 = 0x666666;
pub const TERMINAL_ANSI_9: u32 = 0xff6e64;
pub const TERMINAL_ANSI_10: u32 = 0xb7e2a3;
pub const TERMINAL_ANSI_11: u32 = 0xf2d47c;
pub const TERMINAL_ANSI_12: u32 = 0x9dc7f2;
pub const TERMINAL_ANSI_13: u32 = 0xe0aede;
pub const TERMINAL_ANSI_14: u32 = 0x93e1e2;
pub const TERMINAL_ANSI_15: u32 = 0xffffff;
pub const CURSOR: u32 = TERMINAL_CURSOR;
pub const CURSOR_TEXT: u32 = CANVAS;
pub const ANSI: [u32; 16] = [
    TERMINAL_ANSI_0,
    TERMINAL_ANSI_1,
    TERMINAL_ANSI_2,
    TERMINAL_ANSI_3,
    TERMINAL_ANSI_4,
    TERMINAL_ANSI_5,
    TERMINAL_ANSI_6,
    TERMINAL_ANSI_7,
    TERMINAL_ANSI_8,
    TERMINAL_ANSI_9,
    TERMINAL_ANSI_10,
    TERMINAL_ANSI_11,
    TERMINAL_ANSI_12,
    TERMINAL_ANSI_13,
    TERMINAL_ANSI_14,
    TERMINAL_ANSI_15,
];

/// Apps SDK UI roles whose dark value Coder Noir replaces: `(token, noir)`.
/// The light side stays the Apps SDK UI value (Coder Light).
pub const OVERRIDES: &[(&str, u32)] = &[
    // Content ladder
    ("--color-text", CONTENT),
    ("--color-text-secondary", CONTENT_SECONDARY),
    ("--color-text-tertiary", CONTENT_TERTIARY),
    ("--color-text-inverse", ACCENT_ON_SOLID),
    // Focus ring: Noir keeps a neutral accent
    ("--color-ring", ACCENT),
    // Neutral accent (primary solid)
    ("--color-background-primary-solid", ACCENT_SOLID),
    // Strokes
    ("--color-border", STROKE_SUBTLE),
    ("--color-border-strong", STROKE),
    // Canvas and surfaces
    ("--color-surface", CANVAS),
    ("--color-surface-secondary", SURFACE_SUBTLE),
    ("--color-surface-tertiary", SURFACE),
    ("--color-surface-elevated", SURFACE_RAISED),
    ("--color-surface-elevated-secondary", CONTROL_ON_OVERLAY),
];
