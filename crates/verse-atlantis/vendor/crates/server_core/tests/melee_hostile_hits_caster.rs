use glam::vec3;

#[test]
fn melee_hostile_hits_caster() {
    let mut s = server_core::ServerState::new();
    // Spawn wizard target
    let wiz = s.ecs.spawn(
        server_core::ActorKind::Wizard,
        server_core::Faction::Wizards,
        server_core::Transform {
            pos: vec3(0.0, 0.6, 0.0),
            yaw: 0.0,
            radius: 0.7,
        },
        server_core::Health { hp: 30, max: 30 },
    );
    // Spawn zombie within contact distance
    let _z = s.spawn_undead(vec3(0.0, 0.6, 1.0), 0.9, 20);
    let hp0 = s.ecs.get(wiz).unwrap().hp.hp;
    // Step several frames to allow melee contact and cooldown
    for _ in 0..20 {
        s.step_authoritative(0.05);
    }
    let hp1 = s.ecs.get(wiz).unwrap().hp.hp;
    assert!(
        hp1 < hp0,
        "wizard HP should drop from zombie melee ({} -> {})",
        hp0,
        hp1
    );
}
