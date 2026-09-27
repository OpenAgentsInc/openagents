use glam::vec3;

#[test]
fn caster_selection_uses_faction_not_kind() {
    let mut s = server_core::ServerState::new();
    // Spawn an NPC wizard (Team::Wizards) and then change its kind label to Zombie
    let wid = s.spawn_wizard_npc(vec3(0.0, 0.6, 0.0));
    if let Some(w) = s.ecs.get_mut(wid) {
        w.kind = server_core::ActorKind::Zombie; // change presentation kind only
    }
    // Place two undead within range so AI has targets
    let _u1 = s.spawn_undead(vec3(0.3, 0.6, 12.0), 0.9, 30);
    let _u2 = s.spawn_undead(vec3(-0.3, 0.6, 14.0), 0.9, 30);
    // Step a few frames to allow AI to face and cast
    for _ in 0..5 {
        s.step_authoritative(0.2);
    }
    // Expect at least one projectile spawned even though 'kind' is not Wizard anymore
    let saw_any = s
        .ecs
        .iter()
        .any(|c| c.projectile.is_some() && c.velocity.is_some());
    assert!(
        saw_any,
        "caster selection should be based on faction/components, not ActorKind label"
    );
}
