use glam::vec3;

// Explosion AoE should consider a small pad and the target's collision
// radius so near-misses still apply damage. This guards against thin-target
// edge cases where the planar center is just outside the raw AoE radius.
#[test]
fn fireball_aoe_radius_pad_hits_thin_target() {
    let mut s = server_core::ServerState::new();
    // Spawn PC far from the explosion center so spawn safety doesn't push the target
    let _pc = s.spawn_pc_at(vec3(100.0, 0.6, 0.0));
    // Spawn an Undead slightly outside a 4.0m raw radius, with a small radius
    // (min capsule radius in AoE path is 0.30m, plus a 0.25m pad → effective ~4.55m)
    let z = s.spawn_undead(vec3(4.2, 0.6, 0.0), 0.2, 40);
    let hp0 = s.ecs.get(z).unwrap().hp.hp;
    // Synthesize an explosion centered at origin with r=4.0m
    let mut ctx = server_core::ecs::schedule::Ctx {
        dt: 0.016,
        ..Default::default()
    };
    ctx.boom.push(server_core::ecs::schedule::ExplodeEvent {
        center_xz: glam::Vec2::new(0.0, 0.0),
        r2: 4.0 * 4.0,
        src: s.pc_actor,
    });
    // Run only the AoE system and apply-damage via the schedule to avoid duplicating logic
    let mut sched = server_core::ecs::schedule::Schedule;
    sched.run(&mut s, &mut ctx);
    let hp1 = s.ecs.get(z).unwrap().hp.hp;
    assert!(
        hp1 < hp0,
        "expected AoE pad to apply damage ({} -> {})",
        hp0,
        hp1
    );
}
