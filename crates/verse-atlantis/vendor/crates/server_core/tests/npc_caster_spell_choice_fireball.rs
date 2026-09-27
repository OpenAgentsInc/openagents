use glam::vec3;

#[test]
fn npc_caster_casts_at_clustered_targets() {
    let mut s = server_core::ServerState::new();
    // Spawn NPC wizard at origin with casting resources and ensure facing +Z
    let wid = s.spawn_wizard_npc(vec3(0.0, 0.6, 0.0));
    if let Some(w) = s.ecs.get_mut(wid) {
        w.tr.yaw = 0.0;
    }
    // Spawn two undead clustered around z=14..15m
    let _u1 = s.spawn_undead(vec3(0.2, 0.6, 14.5), 0.9, 30);
    let _u2 = s.spawn_undead(vec3(-0.3, 0.6, 15.2), 0.9, 30);
    // Run a few ticks to allow AI to decide and cast
    for _ in 0..5 {
        s.step_authoritative(0.1);
    }
    // Casting Fireball should reduce mana by ~5 from the wizard's pool
    let mana = s
        .ecs
        .get(wid)
        .and_then(|w| w.pool.map(|p| p.mana))
        .unwrap_or(30);
    assert!(
        mana <= 25,
        "expected wizard mana to drop after casting (got {})",
        mana
    );
}
