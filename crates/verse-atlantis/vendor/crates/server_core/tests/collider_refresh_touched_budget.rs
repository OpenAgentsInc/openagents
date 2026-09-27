#![allow(clippy::unwrap_used)]
use glam::UVec3;
use server_core as sc;

#[test]
fn collider_refresh_uses_touched_queue_only() {
    let mut srv = sc::ServerState::new();
    sc::scene_build::add_demo_ruins_destructible(&mut srv);
    let did = server_core::destructible::state::DestructibleId(1);

    // Pretend these two chunks were meshed this tick:
    srv.destruct_registry
        .touched_this_tick
        .push((did, UVec3::new(0, 0, 0)));
    srv.destruct_registry
        .touched_this_tick
        .push((did, UVec3::new(1, 0, 0)));
    // Budget 1
    srv.destruct_registry.cfg.collider_budget_per_tick = 1;

    sc::systems::destructible::destructible_refresh_colliders(&mut srv);
    // One left for next tick
    assert_eq!(srv.destruct_registry.touched_this_tick.len(), 1);
    sc::systems::destructible::destructible_refresh_colliders(&mut srv);
    assert!(srv.destruct_registry.touched_this_tick.is_empty());
}
