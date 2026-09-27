//! Registry-level world AABB broad-phase tests.
#![cfg(feature = "destruct_debug")]
#![allow(clippy::unwrap_used)]

use core_units::Length;
use glam::{DVec3, UVec3, Vec3};
use server_core::destructible::state::{
    DestructibleId, DestructibleProxy, DestructibleRegistry, WorldAabb,
};
use voxel_proxy::{GlobalId, VoxelGrid, VoxelProxyMeta};

fn mk_grid() -> VoxelGrid {
    let meta = VoxelProxyMeta {
        object_id: GlobalId(1),
        origin_m: DVec3::ZERO,
        voxel_m: Length::meters(1.0),
        dims: UVec3::new(16, 16, 16),
        chunk: UVec3::new(8, 8, 8),
        material: core_materials::find_material_id("stone").unwrap(),
    };
    VoxelGrid::new(meta)
}

#[test]
fn seg_intersects_proxy_true_and_false() {
    let did = DestructibleId(42);
    let aabb = WorldAabb {
        min: Vec3::new(-1.0, 0.0, -1.0),
        max: Vec3::new(1.0, 2.0, 1.0),
    };

    let proxy = DestructibleProxy::new(did, mk_grid(), aabb);
    let mut reg = DestructibleRegistry::default();
    reg.insert_proxy(proxy);

    // Horizontal segment through the AABB at y=1
    let p0 = Vec3::new(-2.0, 1.0, 0.0);
    let p1 = Vec3::new(2.0, 1.0, 0.0);
    assert!(reg.seg_intersects_proxy(did, p0, p1));

    // Same x-range but above the AABB
    let p0_hi = Vec3::new(-2.0, 3.0, 0.0);
    let p1_hi = Vec3::new(2.0, 3.0, 0.0);
    assert!(!reg.seg_intersects_proxy(did, p0_hi, p1_hi));
}
