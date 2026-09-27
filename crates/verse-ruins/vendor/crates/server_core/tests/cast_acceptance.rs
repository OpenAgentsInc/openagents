use glam::vec3;

#[test]
fn pc_firebolt_twice_after_gcd() {
    let mut s = server_core::ServerState::new();
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    // First cast accepted
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    s.step_authoritative(0.016);
    let proj_after_first = s.ecs.iter().filter(|a| a.projectile.is_some()).count();
    assert!(proj_after_first > 0, "expected projectile after first cast");
    // Immediate re-cast should be rejected by GCD
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    s.step_authoritative(0.016);
    let proj_after_second = s.ecs.iter().filter(|a| a.projectile.is_some()).count();
    assert!(
        proj_after_second == proj_after_first,
        "GCD should block immediate re-cast"
    );
    // Advance beyond GCD and cast again
    for _ in 0..22 {
        s.step_authoritative(0.016);
    }
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    s.step_authoritative(0.016);
    let proj_after_third = s.ecs.iter().filter(|a| a.projectile.is_some()).count();
    assert!(
        proj_after_third > proj_after_second,
        "expected another projectile after GCD elapsed"
    );
}

#[test]
fn pc_magic_missile_cd_and_mana() {
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
    assert!(mana_after <= 18, "MM should cost ~2 mana");
    // Immediate retry rejected by per-spell CD
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::MagicMissile,
    );
    s.step_authoritative(0.016);
    let mana_blocked = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    assert_eq!(
        mana_blocked, mana_after,
        "per-spell CD blocks immediate MM re-cast"
    );
    // Advance time ~1.6s; retry should accept and deduct mana again
    for _ in 0..110 {
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
        "MM accepted after CD; mana reduced"
    );
}

#[test]
fn pc_fireball_cd_and_mana() {
    let mut s = server_core::ServerState::new();
    let _pc = s.spawn_pc_at(glam::vec3(0.0, 0.6, 0.0));
    // First Fireball: accept; mana -5
    s.enqueue_cast(
        glam::vec3(0.0, 0.6, 0.0),
        glam::vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Fireball,
    );
    s.step_authoritative(0.016);
    let mana_after = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    assert!(
        mana_after <= 15,
        "Fireball should cost ~5 mana (<=15), got {}",
        mana_after
    );
    // Immediate retry should be rejected by per-spell CD (~4s)
    s.enqueue_cast(
        glam::vec3(0.0, 0.6, 0.0),
        glam::vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Fireball,
    );
    s.step_authoritative(0.016);
    let mana_blocked = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    assert_eq!(
        mana_blocked, mana_after,
        "per-spell Fireball CD should block immediate re-cast"
    );
    // Advance ~4.1s to clear CD; mana should also regen ~4.1
    for _ in 0..260 {
        s.step_authoritative(0.016);
    }
    let mana_before_retry = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    s.enqueue_cast(
        glam::vec3(0.0, 0.6, 0.0),
        glam::vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Fireball,
    );
    s.step_authoritative(0.016);
    let mana_after_retry = s.ecs.get(s.pc_actor.unwrap()).unwrap().pool.unwrap().mana;
    assert!(
        mana_after_retry < mana_before_retry,
        "Fireball accepted after CD; mana reduced"
    );
}
