#![allow(clippy::unwrap_used)]
use server_core as sc;

/// An explosion near a wall should only carve if a voxel ray hits that surface;
/// far-side or out-of-range explosions must not carve.
#[test]
fn explosion_surface_pick_hits_face_only() {
    let mut srv = sc::ServerState::new();
    sc::scene_build::add_demo_ruins_destructible(&mut srv);
    let inst = srv.destruct_instances[0].clone();

    let min = glam::Vec3::from(inst.world_min);
    let max = glam::Vec3::from(inst.world_max);

    // Just outside the -X wall, within radius
    let boom_center_outside = glam::vec2(min.x - 0.25, (min.z + max.z) * 0.5);
    let mut ctx = sc::ecs::schedule::Ctx::default();
    ctx.boom.push(sc::ecs::schedule::ExplodeEvent {
        center_xz: boom_center_outside,
        r2: 1.5 * 1.5,
        src: None,
    });

    sc::ecs::schedule::destructible_from_explosions_for_test(&mut srv, &mut ctx);
    assert!(
        !ctx.carves.is_empty(),
        "carve should be produced for surface-adjacent explosion"
    );

    // Place far away (should not carve)
    ctx.carves.clear();
    let far_center = glam::vec2(max.x + 50.0, max.z + 50.0);
    ctx.boom.clear();
    ctx.boom.push(sc::ecs::schedule::ExplodeEvent {
        center_xz: far_center,
        r2: 1.5 * 1.5,
        src: None,
    });
    sc::ecs::schedule::destructible_from_explosions_for_test(&mut srv, &mut ctx);
    assert!(ctx.carves.is_empty(), "far-away explosion must not carve");
}
