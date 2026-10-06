//! Headless Verse Engine asset, animation, and cinematic contracts.
//!
//! This core has no GPU, platform, game rules, transport, or credentials.
pub mod animation;
pub mod animation_graph;
pub mod assets;
pub mod audio;
pub mod audio_bank;
pub mod audio_cues;
pub mod audio_stream;
pub mod core;
pub mod director;
pub mod environment;
pub mod inventory;
pub mod lighting;
#[cfg(feature = "asset-io")]
pub mod loading;
pub mod locomotion;
pub mod markers;
pub mod material;
pub mod mips;
pub mod motion;
pub mod overlay;
pub mod presentation;
pub mod quality;
pub mod render_graph;
pub mod render_world;
pub mod residency;
pub mod sockets;
pub mod streaming;

/// Converts version-one pack source coordinates to Y-up meters.
///
/// The retained compiled-pack convention is Z-up with 0.9144 meters per unit.
/// Original procedural geometry converts into this convention at compilation.
#[must_use]
pub fn source_position(position: [f32; 3]) -> glam::Vec3 {
    glam::Vec3::new(-position[1], position[2], -position[0]) * 0.9144
}
