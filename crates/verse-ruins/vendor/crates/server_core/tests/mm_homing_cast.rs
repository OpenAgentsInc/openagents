use glam::vec3;
use server_core::{ServerState, SpellId};

fn count_projectiles(srv: &ServerState) -> usize {
    srv.ecs.iter().filter(|c| c.projectile.is_some()).count()
}

#[test]
fn magic_missile_spawns_three_and_distinct_targets() {
    let mut s = ServerState::new();
    // PC
    let pc = vec3(0.0, 0.6, 0.0);
    s.sync_wizards(&[pc]);
    // Three undead within 10m
    let _z1 = s.spawn_undead(vec3(6.0, 0.6, 0.0), 0.9, 20);
    let _z2 = s.spawn_undead(vec3(-6.0, 0.6, 0.0), 0.9, 20);
    let _z3 = s.spawn_undead(vec3(0.0, 0.6, 6.0), 0.9, 20);
    // Cast MM toward +Z
    s.enqueue_cast(pc, vec3(0.0, 0.0, 1.0), SpellId::MagicMissile);
    s.step_authoritative(0.01);

    // Ensure 3 projectiles spawned and each has a homing target; distinct if available
    let homing_targets: Vec<_> = s
        .ecs
        .iter()
        .filter(|c| c.projectile.is_some())
        .filter_map(|c| c.homing.as_ref().map(|h| h.target))
        .collect();
    assert!(
        homing_targets.len() >= 3,
        "expected 3 homing projectiles, got {}",
        homing_targets.len()
    );
    use std::collections::HashSet;
    let set: HashSet<_> = homing_targets.iter().cloned().collect();
    assert!(set.len() >= 3, "targets not distinct: {:?}", homing_targets);
}

#[test]
fn homing_steers_toward_target_over_time() {
    let mut s = ServerState::new();
    // PC and one undead at +X
    let pc = vec3(0.0, 0.6, 0.0);
    s.sync_wizards(&[pc]);
    let _z = s.spawn_undead(vec3(12.0, 0.6, 0.0), 0.9, 20);
    // Cast MM initially aimed away (-Z)
    s.enqueue_cast(pc, vec3(0.0, 0.0, -1.0), SpellId::MagicMissile);
    s.step_authoritative(0.01);
    // Read one projectile with homing
    let mut found = None;
    for c in s.ecs.iter() {
        if c.projectile.is_some() && c.homing.is_some() {
            found = Some((c.tr.pos, c.velocity.unwrap().v));
            break;
        }
    }
    let (p0, v0) = found.expect("missing homing projectile");
    // Desired direction is toward +X
    let desired = (vec3(12.0, 0.6, 0.0) - p0).normalize_or_zero();
    let angle0 = v0.normalize_or_zero().angle_between(desired);
    // Step a few frames, then measure angle again
    for _ in 0..5 {
        s.step_authoritative(0.02);
    }
    let mut v1 = None;
    for c in s.ecs.iter() {
        if c.projectile.is_some() && c.homing.is_some() {
            v1 = Some(c.velocity.unwrap().v);
            break;
        }
    }
    let v1 = v1.expect("homing projectile disappeared too early");
    let angle1 = v1.normalize_or_zero().angle_between(desired);
    assert!(
        angle1 < angle0,
        "homing did not reduce angle: {} -> {}",
        angle0,
        angle1
    );
}

#[test]
fn cast_cooldowns_and_mana_gate_repeated_casts() {
    let mut s = ServerState::new();
    let pc = vec3(0.0, 0.6, 0.0);
    s.sync_wizards(&[pc]);
    // Capture initial mana
    let mana0 = s
        .ecs
        .iter()
        .find(|c| c.faction == server_core::Faction::Pc)
        .and_then(|c| c.pool.as_ref().map(|p| p.mana))
        .unwrap_or(0);
    // Cast Fireball (cost=5)
    s.enqueue_cast(pc, vec3(0.0, 0.0, 1.0), SpellId::Fireball);
    s.step_authoritative(0.01);
    // Ensure gcd set and per-spell cooldown started, mana reduced
    let (gcd_ready, cd_rem, mana1) = {
        let c = s
            .ecs
            .iter()
            .find(|c| c.faction == server_core::Faction::Pc)
            .expect("pc present");
        let gcd = c.cooldowns.as_ref().map(|cd| cd.gcd_ready).unwrap_or(0.0);
        let cd = c
            .cooldowns
            .as_ref()
            .and_then(|cd| cd.per_spell.get(&SpellId::Fireball).copied())
            .unwrap_or(0.0);
        let m = c.pool.as_ref().map(|p| p.mana).unwrap_or(0);
        (gcd, cd, m)
    };
    assert!(gcd_ready > 0.25, "gcd not applied: {}", gcd_ready);
    assert!(cd_rem > 3.5, "per-spell cooldown not applied: {}", cd_rem);
    assert_eq!(mana0 - mana1, 5, "mana not debited by cost");
    // Attempt immediate re-cast; should be gated by GCD
    let before = count_projectiles(&s);
    s.enqueue_cast(pc, vec3(0.0, 0.0, 1.0), SpellId::Fireball);
    s.step_authoritative(0.01);
    let after = count_projectiles(&s);
    assert_eq!(before, after, "projectile count changed while on GCD");
}
