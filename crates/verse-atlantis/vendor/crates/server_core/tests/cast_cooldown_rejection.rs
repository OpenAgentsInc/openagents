#![allow(clippy::unwrap_used)]

use server_core as sc;

#[test]
fn fireball_rejected_when_on_cooldown() {
    let mut s = sc::ServerState::new();
    let _pc = s.spawn_pc_at(glam::vec3(0.0, 0.6, 0.0));

    // Plenty of mana
    {
        let id = s.pc_actor.unwrap();
        let e = s.ecs.get_mut(id).unwrap();
        e.pool.as_mut().unwrap().mana = 999;
    }

    let mut ctx = sc::ecs::schedule::Ctx::default();
    let pos = glam::vec3(0.0, 0.6, 0.0);
    let dir = glam::vec3(0.0, 0.0, 1.0);

    // First cast should succeed
    s.enqueue_cast(pos, dir, sc::SpellId::Fireball);
    sc::ecs::schedule::cast_system(&mut s, &mut ctx);
    sc::ecs::schedule::ingest_projectile_spawns_for_test(&mut s, &mut ctx);
    let proj_after_first = s.ecs.iter().filter(|e| e.projectile.is_some()).count();

    // Immediately try second cast before cooldown elapses
    s.enqueue_cast(pos, dir, sc::SpellId::Fireball);
    sc::ecs::schedule::cast_system(&mut s, &mut ctx);
    sc::ecs::schedule::ingest_projectile_spawns_for_test(&mut s, &mut ctx);
    let proj_after_second = s.ecs.iter().filter(|e| e.projectile.is_some()).count();
    assert_eq!(
        proj_after_second, proj_after_first,
        "no new projectile should spawn while on cooldown"
    );
}
