//! The static world a zone builds: geometry uploaded once and the footprints
//! the player cannot walk through.

use verse_pbr::mesh::Mesh;
use verse_world::social::controller::Footprint;

/// The static world: its geometry and what the player cannot walk through.
#[derive(Clone, Debug, Default)]
pub struct World {
    /// Lines and faces uploaded once.
    pub mesh: Mesh,
    /// Building and pylon footprints.
    pub blockers: Vec<Footprint>,
}
