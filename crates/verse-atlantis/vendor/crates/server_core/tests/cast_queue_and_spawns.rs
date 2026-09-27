use glam::vec3;

#[test]
fn cast_queue_drains_and_spawns_projectiles() {
    let mut s = server_core::ServerState::new();
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    // Enqueue three casts in one tick
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.2, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(-0.2, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    s.step_authoritative(0.016);
    assert!(
        s.pending_casts.is_empty(),
        "cast queue should drain each tick"
    );
    // Ingest happens same frame; at least one projectile should exist
    assert!(s.ecs.iter().any(|a| a.projectile.is_some()));
}

#[test]
fn pc_cast_spawns_projectiles() {
    let mut s = server_core::ServerState::new();
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    // Cast FB, MM, FB
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    s.step_authoritative(0.016);
    // Advance GCD
    for _ in 0..20 {
        s.step_authoritative(0.016);
    }
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::MagicMissile,
    );
    s.step_authoritative(0.016);
    assert!(s.ecs.iter().any(|a| a.projectile.is_some()));
}
