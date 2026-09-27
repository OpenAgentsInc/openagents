use glam::Vec3;

#[test]
fn stunned_undead_neither_moves_nor_melees() {
    let mut s = server_core::ServerState::new();
    // PC wizard 1m away so melee would trigger if unstunned
    s.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0)]);
    let z = s.spawn_undead(Vec3::new(0.0, 0.6, 1.2), 0.9, 30);

    // Stun the undead for long enough to cover the frame we test
    if let Some(a) = s.ecs.get_mut(z) {
        a.apply_stun(1.0);
    }

    // Snapshot wizard hp and zombie position
    let wiz_id = s
        .ecs
        .iter()
        .find(|c| {
            matches!(c.kind, server_core::ActorKind::Wizard)
                && c.faction == server_core::Faction::Pc
        })
        .unwrap()
        .id;
    let hp0 = s.ecs.get(wiz_id).unwrap().hp.hp;
    let p0 = s.ecs.get(z).unwrap().tr.pos;

    // Run 0.2s (movement + melee would otherwise occur)
    for _ in 0..2 {
        let _wiz: Vec<Vec3> = s
            .ecs
            .iter()
            .filter(|a| {
                matches!(a.kind, server_core::ActorKind::Wizard)
                    && a.faction == server_core::Faction::Pc
            })
            .map(|a| a.tr.pos)
            .collect();
        s.step_authoritative(0.1);
    }

    let hp1 = s.ecs.get(wiz_id).unwrap().hp.hp;
    let p1 = s.ecs.get(z).unwrap().tr.pos;

    // No damage dealt and no position change beyond tiny epsilon
    assert_eq!(hp0, hp1, "stunned undead should not deal melee damage");
    assert!((p1 - p0).length() <= 1e-3, "stunned undead should not move");
}
