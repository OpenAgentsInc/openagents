use glam::vec3;

#[test]
fn projectile_owner_is_caster() {
    let mut s = server_core::ServerState::new();
    // Spawn a PC (not used as caster here)
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, -2.0));
    // Spawn a wizard NPC as the caster (with casting resources)
    let wid = s.spawn_wizard_npc(vec3(0.0, 0.6, 0.0));
    // Enqueue a cast with explicit caster id
    s.pending_casts.push(server_core::CastCmd {
        pos: vec3(0.0, 0.6, 0.0),
        dir: vec3(0.0, 0.0, 1.0),
        spell: server_core::SpellId::Firebolt,
        caster: Some(wid),
    });
    // Step once to run cast_system and ingest projectiles
    s.step_authoritative(0.016);
    // Find projectile in ECS and verify owner
    let mut found = false;
    for c in s.ecs.iter() {
        if c.projectile.is_some() {
            assert_eq!(
                c.owner.map(|o| o.id),
                Some(wid),
                "projectile owner must be caster"
            );
            found = true;
            break;
        }
    }
    assert!(found, "expected a projectile spawned by cast_system");
}
