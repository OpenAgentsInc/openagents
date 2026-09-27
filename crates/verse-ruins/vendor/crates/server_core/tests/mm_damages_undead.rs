use glam::vec3;

#[test]
fn magic_missile_from_wizard_damages_undead() {
    let mut s = server_core::ServerState::new();
    let wid = s.spawn_wizard_npc(vec3(0.0, 0.6, 0.0));
    let uid = s.spawn_undead(vec3(0.0, 0.6, 8.0), 0.9, 40);
    let hp0 = s.ecs.get(uid).unwrap().hp.hp;
    s.pending_casts.push(server_core::CastCmd {
        pos: vec3(0.0, 0.6, 0.0),
        dir: vec3(0.0, 0.0, 1.0),
        spell: server_core::SpellId::MagicMissile,
        caster: Some(wid),
    });
    for _ in 0..8 {
        s.step_authoritative(0.05);
    }
    let hp1 = s.ecs.get(uid).unwrap().hp.hp;
    assert!(
        hp1 < hp0,
        "undead HP should drop after MagicMissile ({} -> {})",
        hp0,
        hp1
    );
}
