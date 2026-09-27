#![allow(clippy::unwrap_used)]

use glam::vec3;
use server_core as sc;

#[test]
fn dk_never_casts_unknown_firebolt() {
    let mut s = sc::ServerState::new();
    // Spawn DK with Fireball + MagicMissile in spawn_death_knight
    let dk = s.spawn_death_knight(vec3(20.0, 0.6, 0.0));
    // Spawn one undead target near DK so it casts
    let _z = s.spawn_undead(vec3(28.0, 0.6, 0.0), 0.9, 10);

    // Run several ticks of the schedule to let AI cast
    for _ in 0..120 {
        let mut ctx = sc::ecs::schedule::Ctx::default();
        let mut sched = sc::ecs::schedule::Schedule;
        sched.run(&mut s, &mut ctx);
    }
    // Inspect projectiles: DK should never have spawned Firebolt (unknown to its spellbook)
    let mut any_firebolt = false;
    for c in s.ecs.iter() {
        if let Some(p) = c.projectile.as_ref() {
            if p.kind == sc::ProjKind::Firebolt {
                any_firebolt = true;
                break;
            }
        }
    }
    assert!(
        !any_firebolt,
        "DK should not cast Firebolt (not in spellbook)"
    );
}
