use glam::vec3;

#[test]
fn firebolt_from_wizard_damages_undead() {
    let mut s = server_core::ServerState::new();
    // Spawn wizard caster
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
    // Spawn undead target 5m ahead
    let uid = s.spawn_undead(vec3(0.0, 0.6, 5.0), 0.9, 30);
    let hp_before = s.ecs.get(uid).unwrap().hp.hp;
    // Enqueue projectile owned by wizard toward undead
    s.spawn_projectile_from(
        wid,
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::ProjKind::Firebolt,
    );
    // Step a few ticks to travel and arm (>= 0.08s)
    for _ in 0..4 {
        s.step_authoritative(0.05);
    }
    let hp_after = s.ecs.get(uid).unwrap().hp.hp;
    assert!(
        hp_after < hp_before,
        "undead HP should drop on hit ({} -> {})",
        hp_before,
        hp_after
    );
}
