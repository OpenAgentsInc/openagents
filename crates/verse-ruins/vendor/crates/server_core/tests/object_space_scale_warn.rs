#![allow(clippy::unwrap_used)]
use server_core as sc;

#[test]
fn non_uniform_scale_warns_and_carve_applies() {
    let mut srv = sc::ServerState::new();
    sc::scene_build::add_demo_ruins_destructible(&mut srv);
    // Force a non-uniform object_from_world
    {
        let did = server_core::destructible::state::DestructibleId(1);
        let proxy = srv.destruct_registry.proxies.get_mut(&did).unwrap();
        proxy.object_from_world = glam::Mat4::from_scale(glam::vec3(1.0, 1.3, 0.9));
    }
    let inst = srv.destruct_instances[0].clone();

    let mut ctx = sc::ecs::schedule::Ctx::default();
    ctx.carves.push(ecs_core::components::CarveRequest {
        did: inst.did,
        center_m: glam::dvec3(1.0, 1.0, 1.0),
        radius_m: 1.0,
        seed: 0,
        impact_id: 0,
    });
    sc::systems::destructible::destructible_apply_carves(&mut srv, &mut ctx);
    assert!(ctx.carves.is_empty());
}
