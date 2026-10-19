//! CAD Lab models in Everglade's physics, without the renderer (#12183).
//!
//! The website runs the same CAD physics the Verse does
//! (`crates/openagents-web/src/cad_web/world_physics.rs`), so the pieces it
//! needs live here, on `glam`, `serde` and `physics` only: the demolition
//! [`site`], the CAD rigs and their sync messages ([`cad_physics`]), the
//! [`cad_place`] placements, the Everglade ground ([`everglade`],
//! [`everglade_water`]), the Reverse Gravity and Wind Wall forces
//! ([`reverse_gravity`], [`wind_wall`]), and the arrival points of the meteor
//! zones ([`spawns`]). `verse`, `verse-world`, `verse-pbr` and
//! `verse-zone-everglade` re-export each under its old path.

pub mod cad_physics;
pub mod cad_place;
pub mod everglade;
pub mod everglade_water;
pub mod reverse_gravity;
pub mod shapes;
pub mod site;
pub mod spawns;
pub mod wind_wall;

pub use shapes::{Footprint, LitVertex, box_hit};
