#![allow(clippy::unwrap_used)]

use glam::vec3;

#[test]
fn fireball_cast_carves_demo_ruins_end_to_end() {
    let mut s = server_core::ServerState::new();
    // Register one centered demo destructible (shifted +Z in scene_build)
    server_core::scene_build::add_demo_ruins_destructible(&mut s);
    // Spawn PC a few meters south of the ruins and aim toward its center
    let aabb = s
        .destruct_instances
        .first()
        .cloned()
        .expect("demo ruins instance present");
    let min = glam::Vec3::from(aabb.world_min);
    let max = glam::Vec3::from(aabb.world_max);
    let center = (min + max) * 0.5;
    let pc_pos = center + vec3(0.0, 0.6, -6.0);
    let _pc = s.spawn_pc_at(pc_pos);
    let dir = (center - pc_pos).normalize_or_zero();

    // Enqueue Fireball cast from the PC and run the relevant schedule steps
    s.enqueue_cast(pc_pos, dir, server_core::SpellId::Fireball);
    let mut ctx = server_core::ecs::schedule::Ctx::default();
    // Cast validate + projectile spawn
    server_core::ecs::schedule::cast_system(&mut s, &mut ctx);
    server_core::ecs::schedule::ingest_projectile_spawns_for_test(&mut s, &mut ctx);
    // Advance one small tick to move projectile forward and test collisions
    ctx.dt = 0.1;
    server_core::ecs::schedule::projectile_integrate_ecs_for_test(&mut s, &mut ctx);
    server_core::ecs::schedule::projectile_collision_ecs_for_test(&mut s, &mut ctx);
    // Convert projectile/explosion → destructible carves; then apply + mesh
    server_core::ecs::schedule::destructible_from_projectiles_for_test(&mut s, &mut ctx);
    server_core::ecs::schedule::destructible_from_explosions_for_test(&mut s, &mut ctx);
    server_core::systems::destructible::destructible_apply_carves(&mut s, &mut ctx);
    server_core::systems::destructible::destructible_remesh_budgeted(&mut s);

    let deltas = s.drain_destruct_mesh_deltas();
    assert!(
        !deltas.is_empty(),
        "expected at least one ChunkMeshDelta after Fireball impacts ruins"
    );
}
