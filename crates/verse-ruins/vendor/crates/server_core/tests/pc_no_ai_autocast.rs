use glam::vec3;

// The player-controlled PC must never be driven by caster AI.
// Verify that, even in the presence of hostiles, the PC does not consume mana
// unless an explicit cast is enqueued.
#[test]
fn pc_does_not_autocast_or_consume_mana() {
    let mut s = server_core::ServerState::new();
    let pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    // Put an undead in range so AI would have a target if it ran on PC
    let _z = s.spawn_undead(vec3(5.0, 0.6, 0.0), 0.9, 20);
    // Start with a known mana and disable regen to isolate consumption
    {
        let a = s.ecs.get_mut(pc).unwrap();
        let p = a.pool.as_mut().unwrap();
        p.mana = 12;
        p.regen_per_s = 0.0;
    }
    // Step for a couple seconds of sim; if AI incorrectly drives the PC, mana would drop
    for _ in 0..120 {
        s.step_authoritative(1.0 / 60.0);
    }
    let mana = s.ecs.get(pc).unwrap().pool.as_ref().unwrap().mana;
    assert_eq!(
        mana, 12,
        "PC mana should remain unchanged without input casts"
    );
}
