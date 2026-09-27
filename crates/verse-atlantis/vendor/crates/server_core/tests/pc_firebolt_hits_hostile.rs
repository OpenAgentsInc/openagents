use glam::vec3;

#[test]
fn pc_firebolt_hits_hostile() {
    let mut s = server_core::ServerState::new();
    // Spawn PC and a Wizard target
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    let wiz = s.ecs.spawn(
        server_core::ActorKind::Wizard,
        server_core::Faction::Wizards,
        server_core::Transform {
            pos: vec3(0.0, 0.6, 6.0),
            yaw: 0.0,
            radius: 0.7,
        },
        server_core::Health { hp: 50, max: 50 },
    );
    // PC casts Firebolt toward wizard
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    // Step to arm and collide
    for _ in 0..6 {
        s.step_authoritative(0.05);
    }
    let hp_after = s.ecs.get(wiz).unwrap().hp.hp;
    assert!(
        hp_after < 50,
        "wizard HP should drop after PC Firebolt: {}",
        hp_after
    );
}
