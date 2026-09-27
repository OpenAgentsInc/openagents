#![cfg(feature = "destruct_debug")]
#![allow(clippy::unwrap_used)]

use glam::UVec3;
use server_core::destructible::state::{
    DestructibleId, DestructibleProxy, DestructibleRegistry, WorldAabb,
};
use server_core::systems::destructible::destructible_refresh_colliders;
use voxel_proxy::{GlobalId, VoxelGrid, VoxelProxyMeta};

#[test]
fn collider_refresh_respects_budget_and_drains_queue() {
    // Build a tiny grid and a registry with one proxy
    let meta = VoxelProxyMeta {
        object_id: GlobalId(1),
        origin_m: glam::DVec3::ZERO,
        voxel_m: core_units::Length::meters(1.0),
        dims: UVec3::new(16, 16, 16),
        chunk: UVec3::new(8, 8, 8),
        material: core_materials::find_material_id("stone").unwrap(),
    };
    let grid = VoxelGrid::new(meta);
    let did = DestructibleId(1);
    let mut reg = DestructibleRegistry::default();
    reg.cfg.collider_budget_per_tick = 1;
    reg.insert_proxy(DestructibleProxy::new(
        did,
        grid,
        WorldAabb {
            min: glam::Vec3::ZERO,
            max: glam::Vec3::splat(1.0),
        },
    ));
    // Simulate two touched chunks this tick
    reg.touched_this_tick.push((did, UVec3::new(0, 0, 0)));
    reg.touched_this_tick.push((did, UVec3::new(1, 0, 0)));
    // First refresh consumes one
    {
        let mut srv = server_core::ServerState::new();
        srv.destruct_registry = reg;
        destructible_refresh_colliders(&mut srv);
        // One should remain deferred
        assert_eq!(srv.destruct_registry.touched_this_tick.len(), 1);
        // Drain second
        destructible_refresh_colliders(&mut srv);
        assert_eq!(srv.destruct_registry.touched_this_tick.len(), 0);
        // Move back registry
        reg = srv.destruct_registry;
    }
}
