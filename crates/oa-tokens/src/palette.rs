//! The native role palettes, Coder Light and Coder Noir: the token table
//! resolved to plain 8-bit colors for GUI surfaces that do not run CSS
//! (Rust Native on desktop and mobile).
//!
//! Each role names the Apps SDK UI token it comes from ([`Palette::SOURCES`]).
//! [`Palette::LIGHT`] is that token's light value (Coder Light) and
//! [`Palette::NOIR`] is the [`crate::noir`] value, which is the token's dark
//! value wherever Noir overrides the role. Tests hold both to the table, so
//! the web and the native apps cannot drift apart.

use crate::{Scheme, noir};

/// An 8-bit sRGB color with straight alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    /// An opaque color from `0xRRGGBB`.
    #[must_use]
    pub const fn rgb(value: u32) -> Rgba8 {
        Rgba8 {
            r: (value >> 16) as u8,
            g: (value >> 8) as u8,
            b: value as u8,
            a: 255,
        }
    }

    /// This color at `alpha` (0 to 255).
    #[must_use]
    pub const fn with_alpha(self, alpha: u8) -> Rgba8 {
        Rgba8 { a: alpha, ..self }
    }

    /// `0xRRGGBB`, alpha dropped.
    #[must_use]
    pub const fn rgb_u32(self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | self.b as u32
    }
}

/// The colors a GUI surface paints with, by role. The names follow
/// `coder_noir` (Coder Noir's roles); each documents its token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// The window's field. `--color-surface`.
    pub canvas: Rgba8,
    /// A quiet panel on the canvas, such as a sidebar. `--color-surface-secondary`.
    pub surface_subtle: Rgba8,
    /// A panel or well. `--color-surface-tertiary`.
    pub surface: Rgba8,
    /// A card, menu, or dialog above the canvas. `--color-surface-elevated`.
    pub surface_raised: Rgba8,
    /// A control drawn on a raised surface. `--color-surface-elevated-secondary`.
    pub control_on_overlay: Rgba8,
    /// Hairlines and quiet borders. `--color-border`.
    pub stroke_subtle: Rgba8,
    /// Strong borders. `--color-border-strong`.
    pub stroke: Rgba8,
    /// Primary text. `--color-text`.
    pub content: Rgba8,
    /// Secondary text. `--color-text-secondary`.
    pub content_secondary: Rgba8,
    /// Receded text: placeholders, meta. `--color-text-tertiary`.
    pub content_tertiary: Rgba8,
    /// The focus ring and interactive accent. `--color-ring`.
    pub accent: Rgba8,
    /// A primary button's fill. `--color-background-primary-solid`.
    pub accent_solid: Rgba8,
    /// A primary button's label. `--color-text-inverse`.
    pub accent_on_solid: Rgba8,
    /// Danger text. `--color-text-danger`.
    pub danger: Rgba8,
    /// Success text. `--color-text-success`.
    pub success: Rgba8,
    /// Warning text. `--color-text-warning`.
    pub warning: Rgba8,
    /// Informational text. `--color-text-info`.
    pub info: Rgba8,
    /// Discovery (purple) text. `--color-text-discovery`; Noir has no
    /// discovery role and uses its terminal magenta.
    pub discovery: Rgba8,
    /// A danger wash behind content. `--color-background-danger-soft`.
    pub danger_container: Rgba8,
    /// A success wash. `--color-background-success-soft`.
    pub success_container: Rgba8,
    /// A warning wash. `--color-background-warning-soft`.
    pub warning_container: Rgba8,
    /// An informational wash. `--color-background-info-soft`.
    pub info_container: Rgba8,
}

impl Palette {
    /// Coder Light: Apps SDK UI's light values.
    pub const LIGHT: Palette = Palette {
        canvas: Rgba8::rgb(0xffffff),
        surface_subtle: Rgba8::rgb(0xf9f9f9),
        surface: Rgba8::rgb(0xf3f3f3),
        surface_raised: Rgba8::rgb(0xffffff),
        control_on_overlay: Rgba8::rgb(0xf9f9f9),
        // alpha(var(--alpha-base), 10%) and 15% over #0d0d0d.
        stroke_subtle: Rgba8::rgb(0x0d0d0d).with_alpha(26),
        stroke: Rgba8::rgb(0x0d0d0d).with_alpha(38),
        content: Rgba8::rgb(0x0d0d0d),
        content_secondary: Rgba8::rgb(0x5d5d5d),
        content_tertiary: Rgba8::rgb(0x8f8f8f),
        accent: Rgba8::rgb(0x0169cc),
        accent_solid: Rgba8::rgb(0x181818),
        accent_on_solid: Rgba8::rgb(0xffffff),
        danger: Rgba8::rgb(0x911e1b),
        success: Rgba8::rgb(0x00692a),
        warning: Rgba8::rgb(0x923b0f),
        info: Rgba8::rgb(0x0169cc),
        discovery: Rgba8::rgb(0x532d8d),
        danger_container: Rgba8::rgb(0xffd9d9),
        success_container: Rgba8::rgb(0xd9f4e4),
        warning_container: Rgba8::rgb(0xffe7d9),
        info_container: Rgba8::rgb(0xe5f3ff),
    };

    /// Coder Noir.
    pub const NOIR: Palette = Palette {
        canvas: Rgba8::rgb(noir::CANVAS),
        surface_subtle: Rgba8::rgb(noir::SURFACE_SUBTLE),
        surface: Rgba8::rgb(noir::SURFACE),
        surface_raised: Rgba8::rgb(noir::SURFACE_RAISED),
        control_on_overlay: Rgba8::rgb(noir::CONTROL_ON_OVERLAY),
        stroke_subtle: Rgba8::rgb(noir::STROKE_SUBTLE),
        stroke: Rgba8::rgb(noir::STROKE),
        content: Rgba8::rgb(noir::CONTENT),
        content_secondary: Rgba8::rgb(noir::CONTENT_SECONDARY),
        content_tertiary: Rgba8::rgb(noir::CONTENT_TERTIARY),
        accent: Rgba8::rgb(noir::ACCENT),
        accent_solid: Rgba8::rgb(noir::ACCENT_SOLID),
        accent_on_solid: Rgba8::rgb(noir::ACCENT_ON_SOLID),
        danger: Rgba8::rgb(noir::DANGER),
        success: Rgba8::rgb(noir::SUCCESS),
        warning: Rgba8::rgb(noir::WARNING),
        info: Rgba8::rgb(noir::INFO),
        discovery: Rgba8::rgb(noir::TERMINAL_ANSI_5),
        danger_container: Rgba8::rgb(noir::DANGER_CONTAINER),
        success_container: Rgba8::rgb(noir::SUCCESS_CONTAINER),
        warning_container: Rgba8::rgb(noir::WARNING_CONTAINER),
        info_container: Rgba8::rgb(noir::INFO_CONTAINER),
    };

    /// Each role's source token, in field order.
    pub const SOURCES: [&'static str; 22] = [
        "--color-surface",
        "--color-surface-secondary",
        "--color-surface-tertiary",
        "--color-surface-elevated",
        "--color-surface-elevated-secondary",
        "--color-border",
        "--color-border-strong",
        "--color-text",
        "--color-text-secondary",
        "--color-text-tertiary",
        "--color-ring",
        "--color-background-primary-solid",
        "--color-text-inverse",
        "--color-text-danger",
        "--color-text-success",
        "--color-text-warning",
        "--color-text-info",
        "--color-text-discovery",
        "--color-background-danger-soft",
        "--color-background-success-soft",
        "--color-background-warning-soft",
        "--color-background-info-soft",
    ];

    /// The palette for a scheme.
    #[must_use]
    pub const fn of(scheme: Scheme) -> &'static Palette {
        match scheme {
            Scheme::Light => &Palette::LIGHT,
            Scheme::Dark => &Palette::NOIR,
        }
    }

    /// Every role's color, in [`Palette::SOURCES`] order.
    #[must_use]
    pub const fn roles(&self) -> [Rgba8; 22] {
        [
            self.canvas,
            self.surface_subtle,
            self.surface,
            self.surface_raised,
            self.control_on_overlay,
            self.stroke_subtle,
            self.stroke,
            self.content,
            self.content_secondary,
            self.content_tertiary,
            self.accent,
            self.accent_solid,
            self.accent_on_solid,
            self.danger,
            self.success,
            self.warning,
            self.info,
            self.discovery,
            self.danger_container,
            self.success_container,
            self.warning_container,
            self.info_container,
        ]
    }
}
