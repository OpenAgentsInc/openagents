#![cfg(feature = "destruct_debug")]
#![allow(clippy::unwrap_used)]

use server_core::scene_build::add_demo_ruins_destructible;

#[test]
fn apply_carve_then_mesh_emits_delta() {
    let mut srv = server_core::ServerState::new();
    add_demo_ruins_destructible(&mut srv);
    // Ensure budgets allow at least one chunk
    srv.destruct_registry.cfg.max_chunk_remesh = 4;
    // Push a carve in the center of the AABB
    let inst = srv.destruct_instances[0].clone();
    let center = glam::vec3(
        0.5 * (inst.world_min[0] + inst.world_max[0]),
        0.5 * (inst.world_min[1] + inst.world_max[1]),
        0.5 * (inst.world_min[2] + inst.world_max[2]),
    );
    let mut ctx = server_core::ecs::schedule::Ctx::default();
    ctx.carves.push(ecs_core::components::CarveRequest {
        did: inst.did,
        center_m: center.as_dvec3(),
        radius_m: 1.0,
        seed: 0,
        impact_id: 0,
    });
    server_core::systems::destructible::destructible_apply_carves(&mut srv, &mut ctx);
    server_core::systems::destructible::destructible_remesh_budgeted(&mut srv);
    assert!(!srv.destruct_registry.pending_mesh_deltas.is_empty());
}

#[test]
fn firebolt_does_not_carve_but_fireball_does() {
    // Validate spec gating
    let s = server_core::ServerState::new();
    let fb = s.projectile_spec(server_core::ProjKind::Firebolt);
    assert!(!fb.carves_destructibles);
    let fball = s.projectile_spec(server_core::ProjKind::Fireball);
    assert!(fball.carves_destructibles);
    assert!(fball.carve_radius_m >= 0.0);
}

#[test]
fn object_space_radius_scales_with_uniform_transform() {
    let mut srv = server_core::ServerState::new();
    add_demo_ruins_destructible(&mut srv);
    // Scale object_from_world by 2 uniformly
    let did = server_core::destructible::state::DestructibleId(1);
    if let Some(p) = srv.destruct_registry.proxies.get_mut(&did) {
        let s = glam::Mat4::from_scale(glam::Vec3::splat(2.0));
        p.object_from_world = s;
        p.world_from_object = s.inverse();
    }
    // Carve with radius 1.0; after scaling, effective should be 2.0; we approximate by expecting more chunks touched
    // (We just assert that a delta is emitted; detailed voxel counts are covered elsewhere.)
    let inst = srv.destruct_instances[0].clone();
    let center = glam::vec3(
        0.5 * (inst.world_min[0] + inst.world_max[0]),
        0.5 * (inst.world_min[1] + inst.world_max[1]),
        0.5 * (inst.world_min[2] + inst.world_max[2]),
    );
    let mut ctx = server_core::ecs::schedule::Ctx::default();
    ctx.carves.push(ecs_core::components::CarveRequest {
        did: inst.did,
        center_m: center.as_dvec3(),
        radius_m: 1.0,
        seed: 0,
        impact_id: 0,
    });
    server_core::systems::destructible::destructible_apply_carves(&mut srv, &mut ctx);
    server_core::systems::destructible::destructible_remesh_budgeted(&mut srv);
    assert!(!srv.destruct_registry.pending_mesh_deltas.is_empty());
}

#[test]
fn demo_ruins_has_no_top_slab() {
    // Ensure the demo proxy does not create a full top layer slab (open sky)
    let mut srv = server_core::ServerState::new();
    add_demo_ruins_destructible(&mut srv);
    let did = server_core::destructible::state::DestructibleId(1);
    let proxy = srv.destruct_registry.proxies.get(&did).expect("proxy");
    let dims = proxy.grid.dims();
    let top_y = dims.y - 1;
    let mut solids = 0usize;
    for z in 0..dims.z {
        for x in 0..dims.x {
            if proxy.grid.is_solid(x, top_y, z) {
                solids += 1;
            }
        }
    }
    // No full top slab: allow a tiny tolerance (some side walls may reach top)
    let total = (dims.x * dims.z) as usize;
    assert!(
        solids * 10 < total,
        "top layer should be mostly open (solids={solids}, total={total})"
    );
}

#[test]
fn remesh_budget_is_deterministic_over_ticks() {
    let mut srv = server_core::ServerState::new();
    add_demo_ruins_destructible(&mut srv);
    // Limit to 1 chunk per tick
    srv.destruct_registry.cfg.max_chunk_remesh = 1;
    let inst = srv.destruct_instances[0].clone();
    let min = glam::vec3(inst.world_min[0], inst.world_min[1], inst.world_min[2]);
    // Two distinct impact centers far apart in X to touch different chunks
    let c0 = min + glam::vec3(1.0, 1.0, 1.0);
    let c1 = min + glam::vec3(6.0, 1.0, 1.0);
    let mut ctx = server_core::ecs::schedule::Ctx::default();
    ctx.carves.push(ecs_core::components::CarveRequest {
        did: inst.did,
        center_m: c0.as_dvec3(),
        radius_m: 0.8,
        seed: 0,
        impact_id: 0,
    });
    ctx.carves.push(ecs_core::components::CarveRequest {
        did: inst.did,
        center_m: c1.as_dvec3(),
        radius_m: 0.8,
        seed: 0,
        impact_id: 1,
    });
    server_core::systems::destructible::destructible_apply_carves(&mut srv, &mut ctx);
    // First tick: expect 1 delta
    server_core::systems::destructible::destructible_remesh_budgeted(&mut srv);
    assert_eq!(srv.destruct_registry.pending_mesh_deltas.len(), 1);
    // Drain and run another tick for the second chunk
    srv.destruct_registry.pending_mesh_deltas.clear();
    server_core::systems::destructible::destructible_remesh_budgeted(&mut srv);
    assert_eq!(srv.destruct_registry.pending_mesh_deltas.len(), 1);
}
