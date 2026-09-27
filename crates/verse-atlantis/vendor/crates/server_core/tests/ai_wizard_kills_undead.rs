#![allow(clippy::unwrap_used)]
use server_core as sc;

// Ensure a single NPC wizard can eliminate a lone undead via ranged spells.
#[test]
fn ai_wizard_eliminates_single_undead() {
    let mut s = sc::ServerState::new();
    // Place PC at origin for interest (not used by server logic directly)
    let _pc = s.spawn_pc_at(glam::vec3(0.0, 0.6, 0.0));
    // Spawn one NPC wizard and one undead at moderate distance
    let wiz = s.spawn_wizard_npc(glam::vec3(0.0, 0.6, 0.0));
    let z = s.spawn_undead(glam::vec3(18.0, 0.6, 0.0), 0.95, 60);
    // Run schedule for up to 20 seconds of sim; wizard should kill the zombie
    let mut killed = false;
    let mut hp_last = 60;
    for _ in 0..200 {
        let mut ctx = sc::ecs::schedule::Ctx {
            dt: 0.1,
            ..Default::default()
        };
        let mut sched = sc::ecs::schedule::Schedule;
        sched.run(&mut s, &mut ctx);
        if let Some(zc) = s.ecs.get(z) {
            hp_last = zc.hp.hp;
        } else {
            killed = true; // despawned
            break;
        }
    }
    assert!(
        killed || hp_last == 0,
        "zombie should be eliminated or at 0 HP (last={hp_last})"
    );
    // Sanity: wizard remained alive
    if let Some(wc) = s.ecs.get(wiz) {
        assert!(wc.hp.alive());
    }
}
