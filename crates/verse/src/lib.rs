//! Verse: the OpenAgents desktop world.
//!
//! A first slice: a Tron-style city drawn in amber lines on a near-black
//! field, and a third-person character with World of Warcraft movement and
//! mouselook. The stack follows Ruins of Atlantis (`wgpu`, `winit`, `glam`,
//! a custom renderer). The palette is the Coder terminal's amber ladder and
//! nothing else. Read `docs/verse/README.md`.

pub mod agent;
pub mod app;
pub mod avatar;
pub mod camera;
pub mod controller;
pub mod mesh;
pub mod palette;
pub mod render;
pub mod world;
