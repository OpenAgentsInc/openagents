use glam::vec3;

#[test]
fn arming_delay_prevents_early_hit() {
    let mut s = server_core::ServerState::new();
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
    let uid = s.spawn_undead(vec3(0.0, 0.6, 2.0), 0.9, 30);
    let hp0 = s.ecs.get(uid).unwrap().hp.hp;
    s.spawn_projectile_from(
        wid,
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::ProjKind::Firebolt,
    );
    // Step less than minimal arming time total (< 0.08s)
    s.step_authoritative(0.02);
    s.step_authoritative(0.02);
    s.step_authoritative(0.02);
    let hp_mid = s.ecs.get(uid).unwrap().hp.hp;
    assert_eq!(hp_mid, hp0, "arming delay should prevent early damage");
    // Now step past arming
    s.step_authoritative(0.1);
    let hp1 = s.ecs.get(uid).unwrap().hp.hp;
    assert!(hp1 < hp0, "after arming, damage should apply");
}
