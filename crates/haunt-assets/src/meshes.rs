//! The `mesh` assets, made in code like The Ice Cream Man's.

use toon_mesh::Mesh;

/// The mesh for asset `id` in `state`, centred on its footprint at the
/// floor, facing yaw 0 (north). None for cards, effects, and ids the
/// library doesn't make.
#[must_use]
pub fn build(id: &str, state: &str) -> Option<Mesh> {
    let _ = (id, state);
    None
}
