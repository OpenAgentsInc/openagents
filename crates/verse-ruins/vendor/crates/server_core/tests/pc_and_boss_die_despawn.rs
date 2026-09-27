#![allow(clippy::unwrap_used)]

use glam::vec3;
use server_core as sc;

#[test]
fn pc_death_sets_alive_false_and_despawns() {
    let mut s = sc::ServerState::new();
    let pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    // Apply lethal damage to PC via DamageEvent
    let mut ctx = sc::ecs::schedule::Ctx::default();
    ctx.dmg.push(sc::ecs::schedule::DamageEvent {
        src: None,
        dst: pc,
        amount: 9999,
    });
    sc::ecs::schedule::apply_damage_to_ecs(&mut s, &mut ctx);
    // Cleanup should despawn dead PC
    sc::ecs::schedule::cleanup(&mut s, &mut ctx);
    assert!(s.ecs.get(pc).is_none(), "PC entity should despawn on death");
}

#[test]
fn boss_death_despawns_like_zombie() {
    let mut s = sc::ServerState::new();
    let dk = s.spawn_death_knight(vec3(20.0, 0.6, 0.0));
    // Deal lethal damage
    let mut ctx = sc::ecs::schedule::Ctx::default();
    ctx.dmg.push(sc::ecs::schedule::DamageEvent {
        src: None,
        dst: dk,
        amount: 9999,
    });
    sc::ecs::schedule::apply_damage_to_ecs(&mut s, &mut ctx);
    sc::ecs::schedule::cleanup(&mut s, &mut ctx);
    assert!(
        s.ecs.get(dk).is_none(),
        "Death Knight should despawn on death"
    );
}
