//! Owned Quick_3D_MMORPG simulation, adapted from public MIT code at
//! 547884332ca650abe96264f7230702d36481b9bc. See third_party/quick-mmorpg/NOTICE.

mod data;
mod motor;
pub mod scenery;
pub mod terrain;
mod world;
pub use data::*;
pub use motor::*;
pub use world::*;

pub const DT: f32 = 1.0 / 60.0;
pub const MAX_PLAYERS: usize = 32;
pub const MAX_NPCS: usize = 1024;
pub type V3 = [f32; 3];

pub fn finite_position(position: V3) -> bool {
    position
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 20_000.0)
}
pub fn distance(a: V3, b: V3) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}
pub fn horizontal(a: V3, b: V3) -> f32 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

#[cfg(test)]
mod tests;
