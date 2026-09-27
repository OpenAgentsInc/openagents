#![allow(clippy::unwrap_used)]

use glam::vec3;

#[test]
fn projectiles_are_excluded_from_actor_snapshot() {
    let mut s = server_core::ServerState::new();
    // Spawn a wizard NPC (actor) and then enqueue and ingest a projectile
    let wiz = s.spawn_wizard_npc(vec3(0.0, 0.6, 0.0));
    assert!(s.ecs.get(wiz).is_some());

    // Enqueue a fireball spawn and ingest to ECS
    s.spawn_projectile_from(
        wiz,
        vec3(1.0, 1.2, 0.0),
        vec3(1.0, 0.0, 0.0),
        server_core::ProjKind::Fireball,
    );
    // Ingest pending spawns into ECS via the authoritative schedule
    s.step_authoritative(0.0);

    // Build snapshot and assert actor list only contains true actors (wizard), not the projectile
    let snap = s.tick_snapshot_actors(1);
    // Exactly one actor (the wizard)
    assert_eq!(
        snap.actors.len(),
        1,
        "actor snapshot must exclude projectile entities"
    );
    assert_eq!(snap.actors[0].id, wiz.0);
    // Projectiles are carried separately
    assert_eq!(
        snap.projectiles.len(),
        1,
        "projectile should appear in projectiles list"
    );
}
