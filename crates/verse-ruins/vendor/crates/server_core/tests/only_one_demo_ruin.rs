#![allow(clippy::unwrap_used)]

#[test]
fn demo_server_registers_single_ruin_only() {
    let mut s = server_core::ServerState::new();
    server_core::scene_build::add_demo_ruins_destructible(&mut s);
    // Only one instance should be present
    assert_eq!(s.destruct_instances.len(), 1);
    // Only one proxy should be registered with the registry
    assert_eq!(s.destruct_registry.proxies.len(), 1);
    // The AABB must be on the ground (min.y == 0)
    let a = &s.destruct_instances[0];
    assert!((a.world_min[1] - 0.0).abs() < 1e-6);
}
