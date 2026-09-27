#![allow(clippy::unwrap_used)]
use server_core as sc;

#[test]
fn carve_bus_cap_is_enforced() {
    // Arrange
    let mut srv = sc::ServerState::new();
    sc::scene_build::add_demo_ruins_destructible(&mut srv);
    let inst = srv.destruct_instances[0].clone();

    // Synthesize many carves for the same DID this tick
    let mut ctx = sc::ecs::schedule::Ctx::default();
    let cap = 1024usize; // keep in sync with schedule cap
    for i in 0..(cap * 3) {
        ctx.carves.push(ecs_core::components::CarveRequest {
            did: inst.did,
            center_m: glam::dvec3((i % 8) as f64 + 0.5, 1.0, (i / 8 % 8) as f64 + 0.5),
            radius_m: 0.6,
            seed: 0,
            impact_id: i as u32,
        });
    }

    // Act: apply carves (the system should drop beyond the cap by retaining in ctx)
    sc::systems::destructible::destructible_apply_carves(&mut srv, &mut ctx);

    // Assert - some may be retained for next tick but not exceed cap
    assert!(ctx.carves.len() <= cap);
}
