//! The HUD layer Kiki and Verse draw over the live world: glass panels,
//! hairlines, corner ticks, glow, the top bar, and the small mono labels.
//!
//! Coder Noir has no translucent roles, so these are our own. The web
//! stylesheet writes them as `--hud-*` variables ([`crate::stylesheet`]);
//! native and GPU renderers read the constants.

use crate::Rgba8;

/// The glass behind panels and toasts. `--hud-glass`.
pub const GLASS: Rgba8 = Rgba8::rgb(0x0b0c10).with_alpha(0xcc);
/// Panel and button outlines. `--hud-line`.
pub const LINE: Rgba8 = Rgba8::rgb(0xffffff).with_alpha(0x26);
/// Panel corner ticks. `--hud-tick`.
pub const TICK: Rgba8 = Rgba8::rgb(0xffffff).with_alpha(0x73);
/// The panel shadow: a faint inner hairline and a soft drop. `--hud-glow`.
pub const GLOW: &str = "0 0 0 1px #ffffff14, 0 18px 60px #00000080";
/// The length of a corner tick's arms. `--hud-tick-size`.
pub const TICK_SIZE: &str = "14px";
/// The top bar's height. `--hud-header`.
pub const HEADER: &str = "64px";
/// The top bar's height at phone width ([`PHONE_MAX_WIDTH`]).
pub const HEADER_PHONE: &str = "56px";
/// Panels and cards. `--radius`, shadcn's base radius.
pub const RADIUS_PANEL: &str = "0.75rem";
/// Buttons, toasts, and other controls. `--hud-radius-control`.
pub const RADIUS_CONTROL: &str = "8px";
/// Labels, indexes, and other readouts, in the mono stack. `--hud-label-size`.
pub const LABEL_SIZE: &str = "11px";
/// A label's letter spacing. `--hud-label-tracking`.
pub const LABEL_TRACKING: &str = "0.18em";

/// Widths up to this are phones: panels become sheets, secondary controls hide.
pub const PHONE_MAX_WIDTH: &str = "760px";
