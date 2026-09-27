use glam::Vec3;

#[test]
#[ignore]
fn firebolt_hits_hostile_reliably() {
    unsafe {
        std::env::set_var("RA_SKIP_CAST_GATING", "1");
    }
    let mut s = server_core::ServerState::new();
    // PC and one NPC wizard 5m ahead
    let pc = Vec3::new(0.0, 0.6, 0.0);
    let wiz = Vec3::new(0.0, 0.6, 1.0);
    s.sync_wizards(&[pc, wiz]);

    // Spawn a Firebolt straight toward the NPC wizard
    eprintln!("pc_actor is_some? {}", s.pc_actor.is_some());
    s.spawn_projectile_from_pc(
        pc,
        Vec3::new(0.0, 0.0, 1.0),
        server_core::ProjKind::Firebolt,
    );
    // Step once to ingest spawns
    s.step_authoritative(0.02);
    // Ensure at least one projectile exists after ingestion
    let proj_after_ingest = s.ecs.iter().filter(|a| a.projectile.is_some()).count();
    eprintln!("projectiles after ingest={}", proj_after_ingest);
    assert!(proj_after_ingest > 0, "no projectile spawned after ingest");
    // Step a few more frames to allow travel + collision
    for _ in 0..4 {
        s.step_authoritative(0.02);
    }

    // Inspect state after stepping
    let proj_count = s.ecs.iter().filter(|a| a.projectile.is_some()).count();
    eprintln!("projectiles alive={}", proj_count);
    let npc_id = s
        .ecs
        .iter()
        .filter(|a| {
            matches!(a.kind, server_core::ActorKind::Wizard)
                && a.faction == server_core::Faction::Wizards
        })
        .map(|a| a.id)
        .next()
        .expect("npc wizard present");
    let npc = s.ecs.get(npc_id).unwrap();
    eprintln!(
        "wizard hp={}/{} pos=({:.2},{:.2},{:.2})",
        npc.hp.hp, npc.hp.max, npc.tr.pos.x, npc.tr.pos.y, npc.tr.pos.z
    );
    assert!(
        npc.hp.hp < npc.hp.max,
        "wizard should take damage from Firebolt"
    );
}
