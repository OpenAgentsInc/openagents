#![allow(clippy::unwrap_used)]
use server_core as sc;

#[test]
fn distant_explosion_does_not_carve() {
    let mut srv = sc::ServerState::new();
    sc::scene_build::add_demo_ruins_destructible(&mut srv);
    let inst = srv.destruct_instances[0].clone();

    let mut ctx = sc::ecs::schedule::Ctx::default();
    // Place the boom center ~60m from the AABB projected point
    let center = glam::vec2(
        inst.world_max[0] as f32 + 60.0,
        inst.world_max[2] as f32 + 60.0,
    );
    ctx.boom.push(sc::ecs::schedule::ExplodeEvent {
        center_xz: center,
        r2: 9.0, // 3m radius
        src: None,
    });

    sc::ecs::schedule::destructible_from_explosions_for_test(&mut srv, &mut ctx);
    assert!(ctx.carves.is_empty());
}
