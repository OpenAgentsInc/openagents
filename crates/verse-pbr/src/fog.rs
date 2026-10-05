//! Fog distances the physical renderer and the line renderer share.
//! `verse::render` re-exports them under their old names.

/// Distance where fog starts, in meters.
pub const FOG_START: f32 = 60.0;
/// Distance where fog is total, in meters.
pub const FOG_END: f32 = 250.0;
/// Where the bare world's fog starts, in meters. With nothing but the grid
/// to hide, the fade begins near the player, so the distant grid dims
/// gradually toward the horizon instead of drawing as bright as the near
/// lines.
pub const BARE_FOG_START: f32 = 6.0;
/// Where the bare world's fog is total, in meters: inside the grid's edge,
/// so the grid ends in the field without a visible border.
pub const BARE_FOG_END: f32 = 110.0;
