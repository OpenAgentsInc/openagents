use glam::vec3;

#[test]
fn death_sets_despawn_and_removes_after_timer() {
    let mut s = server_core::ServerState::new();
    // Spawn wizard and a low-HP undead so a single hit kills it
    let wid = s.ecs.spawn(
        server_core::ActorKind::Wizard,
        server_core::Faction::Wizards,
        server_core::Transform {
            pos: vec3(0.0, 0.6, 0.0),
            yaw: 0.0,
            radius: 0.7,
        },
        server_core::Health { hp: 100, max: 100 },
    );
    let uid = s.spawn_undead(vec3(0.0, 0.6, 2.0), 0.9, 5);
    // Hit with a firebolt
    s.spawn_projectile_from(
        wid,
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::ProjKind::Firebolt,
    );
    // Step enough to arm and apply damage, then linger for despawn (timer ~2s)
    for _ in 0..5 {
        s.step_authoritative(0.1);
    }
    // Should be dead and have a despawn timer set (cleanup later)
    let dead_now = s.ecs.get(uid).map(|a| a.hp.hp).unwrap_or(0) == 0;
    assert!(dead_now, "undead should be dead after hit");
    // Run cleanup; ensure entity removed after timer
    for _ in 0..30 {
        s.step_authoritative(0.1);
    }
    assert!(
        s.ecs.get(uid).is_none(),
        "dead entity should despawn after timer"
    );
}
