use glam::vec3;

#[test]
fn firebolt_gcd_enforced_then_allows() {
    let mut s = server_core::ServerState::new();
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    // First cast FB
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    s.step_authoritative(0.016);
    // Record mana (FB cost 0)
    let mana0 = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    // Immediate re-cast should be blocked by GCD (no mana change)
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    s.step_authoritative(0.016);
    let mana1 = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    assert_eq!(mana1, mana0, "GCD should block immediate FB re-cast");
    // Advance time past GCD (~0.32s)
    for _ in 0..20 {
        s.step_authoritative(0.016);
    }
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    s.step_authoritative(0.016);
    let mana2 = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    assert_eq!(
        mana2, mana1,
        "FB cost 0; after GCD second cast should be accepted"
    );
}

#[test]
fn magic_missile_cd_and_mana_enforced() {
    let mut s = server_core::ServerState::new();
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    // Cast once
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::MagicMissile,
    );
    s.step_authoritative(0.016);
    let mana_after = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    assert!(mana_after <= 18, "MM should cost 2 mana");
    // Immediate re-cast should be blocked by per-spell CD (1.5s)
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::MagicMissile,
    );
    s.step_authoritative(0.016);
    let mana_after2 = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    assert_eq!(
        mana_after2, mana_after,
        "per-spell CD should block immediate MM re-cast"
    );
    // Advance time ~1.6s for CD and mana regen
    for _ in 0..100 {
        s.step_authoritative(0.016);
    }
    let mana_before_retry = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::MagicMissile,
    );
    s.step_authoritative(0.016);
    let mana_after_retry = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    assert!(
        mana_after_retry < mana_before_retry,
        "MM should accept and cost mana after CD expires"
    );
}
