use glam::vec3;

#[test]
fn fireball_aoe_hits_hostiles() {
    let mut s = server_core::ServerState::new();
    // Spawn PC at origin
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    // Spawn one wizard at ~6m to guarantee collision/explosion along +Z
    let w1 = s.ecs.spawn(
        server_core::ActorKind::Wizard,
        server_core::Faction::Wizards,
        server_core::Transform {
            pos: vec3(0.0, 0.6, 6.0),
            yaw: 0.0,
            radius: 0.7,
        },
        server_core::Health { hp: 100, max: 100 },
    );
    // Cast Fireball toward cluster
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Fireball,
    );
    // Step forward until collision triggers explosion
    for _ in 0..60 {
        s.step_authoritative(0.016);
    }
    let hp_after = s.ecs.get(w1).map(|a| a.hp.hp).unwrap_or(0);
    assert!(hp_after < 100, "wizard did not take AoE damage");
}
