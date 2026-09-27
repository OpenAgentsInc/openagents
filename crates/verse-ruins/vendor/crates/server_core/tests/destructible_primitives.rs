//! Basic correctness tests for the server-side destructible primitives.
//! These tests exercise pure CPU helpers: DDA raycast, debris carve,
//! deterministic chunk queue ordering.

#![cfg(feature = "destruct_debug")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use core_units::Length;
use glam::{DVec3, UVec3};
use server_core::destructible;
use voxel_proxy::{GlobalId, VoxelGrid, VoxelProxyMeta};

fn mk_grid(d: UVec3, c: UVec3, vox_m: f64) -> VoxelGrid {
    let meta = VoxelProxyMeta {
        object_id: GlobalId(1),
        origin_m: DVec3::ZERO,
        voxel_m: Length::meters(vox_m),
        dims: d,
        chunk: c,
        material: core_materials::find_material_id("stone").unwrap(),
    };
    VoxelGrid::new(meta)
}

#[test]
fn dda_hits_axis_aligned_voxel() {
    let mut g = mk_grid(UVec3::new(16, 16, 16), UVec3::new(8, 8, 8), 1.0);
    g.set(5, 5, 5, true);

    let hit = destructible::raycast_voxels(
        &g,
        DVec3::new(0.0, 5.2, 5.2),
        DVec3::new(1.0, 0.0, 0.0),
        Length::meters(100.0),
    )
    .expect("ray should hit solid voxel");
    assert_eq!(hit.voxel, UVec3::new(5, 5, 5));
}

#[test]
fn dda_diagonal_hits() {
    let mut g = mk_grid(UVec3::new(16, 16, 16), UVec3::new(8, 8, 8), 1.0);
    g.set(7, 7, 7, true);

    let hit = destructible::raycast_voxels(
        &g,
        DVec3::new(0.2, 0.2, 0.2),
        DVec3::new(1.0, 1.0, 1.0),
        Length::meters(100.0),
    )
    .expect("diagonal ray should hit");
    assert_eq!(hit.voxel, UVec3::new(7, 7, 7));
}

#[test]
fn dda_negative_step_boundary_case() {
    // Start just right of a boundary and step negative along X.
    let mut g = mk_grid(UVec3::new(16, 16, 16), UVec3::new(8, 8, 8), 1.0);
    g.set(10, 5, 5, true);

    let hit = destructible::raycast_voxels(
        &g,
        DVec3::new(10.999, 5.2, 5.2),
        DVec3::new(-1.0, 0.0, 0.0),
        Length::meters(100.0),
    )
    .expect("negative step should hit");
    assert_eq!(hit.voxel, UVec3::new(10, 5, 5));
}

#[test]
fn carve_spawns_capped_debris_with_mass() {
    // Fill a small solid block so carving removes voxels.
    let mut g = mk_grid(UVec3::new(16, 16, 16), UVec3::new(8, 8, 8), 0.5);
    for z in 5..10 {
        for y in 5..10 {
            for x in 5..10 {
                g.set(x, y, z, true);
            }
        }
    }

    let out = destructible::carve_and_spawn_debris(
        &mut g,
        DVec3::new(8.0, 8.0, 8.0),
        Length::meters(1.25),
        12345,
        1,
        50, // cap
    );

    assert!(
        out.positions_m.len() <= 50,
        "debris output must respect cap"
    );
    assert_eq!(out.positions_m.len(), out.velocities_mps.len());
    assert_eq!(out.positions_m.len(), out.masses.len());

    // Sanity: stone should be heavier than wood for the same voxel size.
    let wood = core_materials::find_material_id("wood").unwrap();
    let stone = core_materials::find_material_id("stone").unwrap();
    let mw = core_materials::mass_for_voxel(wood, g.voxel_m()).unwrap();
    let ms = core_materials::mass_for_voxel(stone, g.voxel_m()).unwrap();
    assert!(f64::from(ms) > f64::from(mw));
}

#[test]
fn queue_budget_yields_sorted_chunks() {
    use destructible::queue::ChunkQueue;

    let mut q = ChunkQueue::new();
    q.enqueue_many([
        UVec3::new(2, 0, 0),
        UVec3::new(1, 0, 0),
        UVec3::new(1, 0, 1),
    ]);

    let a = q.pop_budget(2);
    assert_eq!(a, vec![UVec3::new(1, 0, 0), UVec3::new(1, 0, 1)]);

    let b = q.pop_budget(2);
    assert_eq!(b, vec![UVec3::new(2, 0, 0)]);
}
