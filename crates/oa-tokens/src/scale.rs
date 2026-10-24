//! The blur, motion, and spacing scales, beside [`crate::typography`]. The
//! web stylesheet writes them as `--hud-*` variables ([`crate::stylesheet`]);
//! native and GPU renderers read the constants.

/// Backdrop blur radii.
pub mod blur {
    /// Behind panels and cards. `--hud-blur`.
    pub const PANEL: &str = "18px";
    /// Behind the top bar. `--hud-blur-bar`.
    pub const BAR: &str = "14px";
}

/// Durations.
pub mod motion {
    /// Hover and press transitions. `--hud-motion`.
    pub const FAST: &str = "0.15s";
    /// A still image fading out when the live view is ready. `--hud-fade`.
    pub const FADE: &str = "0.8s";
    /// One beat of the signal dot. `--hud-pulse`.
    pub const PULSE: &str = "2.4s";
}

/// The spacing scale, `--hud-space-{name}`, smallest first.
pub mod space {
    pub const XS: &str = "4px";
    pub const SM: &str = "8px";
    pub const MD: &str = "12px";
    pub const LG: &str = "16px";
    pub const XL: &str = "24px";
}
