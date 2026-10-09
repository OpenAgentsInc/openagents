//! Coder Noir, the dark theme, mapped onto Apps SDK UI roles.
//!
//! The values are copied as data from `crates/coder-ui/src/coder_noir.rs`
//! (this crate depends only on `maud`). If Noir changes there, update the
//! constants here and regenerate. Terminal colors live in `product.rs`.
//!
//! Per the adoption plan (decision 5), Noir wins where it defines a role:
//! canvas and surfaces, strokes, the content ladder, the neutral accent and
//! the focus ring. Every other role (intents such as danger, caution and
//! discovery, with their soft, outline and ghost states) keeps Apps SDK UI's
//! dark value.

/// `coder_noir::CANVAS`
pub const CANVAS: u32 = 0x0a0a0a;
/// `coder_noir::SURFACE_SUBTLE`
pub const SURFACE_SUBTLE: u32 = 0x101010;
/// `coder_noir::SURFACE`
pub const SURFACE: u32 = 0x0e0e0e;
/// `coder_noir::SURFACE_RAISED`
pub const SURFACE_RAISED: u32 = 0x191919;
/// `coder_noir::CONTROL_ON_OVERLAY`
pub const CONTROL_ON_OVERLAY: u32 = 0x1e1e1e;
/// `coder_noir::STROKE_SUBTLE`
pub const STROKE_SUBTLE: u32 = 0x2c2c2c;
/// `coder_noir::STROKE`
pub const STROKE: u32 = 0x404040;
/// `coder_noir::CONTENT`
pub const CONTENT: u32 = 0xededed;
/// `coder_noir::CONTENT_SECONDARY`
pub const CONTENT_SECONDARY: u32 = 0x818181;
/// `coder_noir::CONTENT_TERTIARY`
pub const CONTENT_TERTIARY: u32 = 0x656565;
/// `coder_noir::ACCENT_SOLID`
pub const ACCENT_SOLID: u32 = 0xededed;
/// `coder_noir::ACCENT_ON_SOLID`
pub const ACCENT_ON_SOLID: u32 = 0x0a0a0a;
/// `coder_noir::ACCENT`
pub const ACCENT: u32 = 0xededed;
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
