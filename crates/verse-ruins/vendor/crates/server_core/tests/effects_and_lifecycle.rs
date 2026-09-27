#![allow(clippy::float_cmp)]

use glam::Vec3;

#[test]
fn burning_ticks_damage_and_expires() {
    let mut s = server_core::ServerState::new();
    s.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0)]);
    let z = s.spawn_undead(Vec3::new(3.0, 0.6, 0.0), 0.9, 50);
    {
        let a = s.ecs.get_mut(z).expect("undead exists");
        a.apply_burning(10, 1.0, None);
    }
    for _ in 0..10 {
        let _wiz: Vec<Vec3> = s
            .ecs
            .iter()
            .filter(|a| {
                matches!(a.kind, server_core::ActorKind::Wizard)
                    && a.faction == server_core::Faction::Pc
            })
            .map(|a| a.tr.pos)
            .collect();
        s.step_authoritative(0.1);
    }
    let a = s.ecs.get(z).expect("still present");
    assert!(
        a.hp.hp <= 41 && a.hp.hp >= 40,
        "expected ~10 damage, hp={}",
        a.hp.hp
    );
    let rem = a.burning.map(|b| b.remaining_s).unwrap_or(0.0);
    assert!(rem <= 1.0e-3, "burning should expire, remaining={}", rem);
}

#[test]
fn slow_scales_effective_speed() {
    let mut s_no_slow = server_core::ServerState::new();
    s_no_slow.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0)]);
    let z0 = s_no_slow.spawn_undead(Vec3::new(0.0, 0.6, 5.0), 0.9, 30);
    let start0 = s_no_slow.ecs.get(z0).unwrap().tr.pos;
    for _ in 0..10 {
        let _wiz: Vec<Vec3> = s_no_slow
            .ecs
            .iter()
            .filter(|a| {
                matches!(a.kind, server_core::ActorKind::Wizard)
                    && a.faction == server_core::Faction::Pc
            })
            .map(|a| a.tr.pos)
            .collect();
        s_no_slow.step_authoritative(0.1);
    }
    let end0 = s_no_slow.ecs.get(z0).unwrap().tr.pos;
    let dist_no_slow = (end0 - start0).length();

    let mut s_slow = server_core::ServerState::new();
    s_slow.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0)]);
    let z1 = s_slow.spawn_undead(Vec3::new(0.0, 0.6, 5.0), 0.9, 30);
    {
        let a = s_slow.ecs.get_mut(z1).unwrap();
        a.apply_slow(0.5, 2.0);
    }
    let start1 = s_slow.ecs.get(z1).unwrap().tr.pos;
    for _ in 0..10 {
        let _wiz: Vec<Vec3> = s_slow
            .ecs
            .iter()
            .filter(|a| {
                matches!(a.kind, server_core::ActorKind::Wizard)
                    && a.faction == server_core::Faction::Pc
            })
            .map(|a| a.tr.pos)
            .collect();
        s_slow.step_authoritative(0.1);
    }
    let end1 = s_slow.ecs.get(z1).unwrap().tr.pos;
    let dist_slow = (end1 - start1).length();
    assert!(
        dist_slow < dist_no_slow * 0.8,
        "slow ineffective: {:.3} vs {:.3}",
        dist_slow,
        dist_no_slow
    );
}

#[test]
fn stun_blocks_cast() {
    use server_core::SpellId;
    let mut s = server_core::ServerState::new();
    s.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0)]);
    if let Some(pc) = s.pc_actor
        && let Some(a) = s.ecs.get_mut(pc)
    {
        a.apply_stun(1.0);
    }
    s.enqueue_cast(
        Vec3::new(0.0, 0.6, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        SpellId::Firebolt,
    );
    let _wiz: Vec<Vec3> = s
        .ecs
        .iter()
        .filter(|a| {
            matches!(a.kind, server_core::ActorKind::Wizard)
                && a.faction == server_core::Faction::Pc
        })
        .map(|a| a.tr.pos)
        .collect();
    s.step_authoritative(0.1);
    let any_proj = s.ecs.iter().any(|c| c.projectile.is_some());
    assert!(!any_proj, "stunned caster should not spawn projectiles");
}

#[test]
fn death_sets_despawn_or_removes_entity() {
    let mut s = server_core::ServerState::new();
    s.sync_wizards(&[glam::vec3(0.0, 0.6, 0.0)]);
    let z = s.spawn_undead(glam::vec3(0.5, 0.6, 0.5), 0.9, 5);
    {
        let a = s.ecs.get_mut(z).unwrap();
        a.apply_burning(50, 0.2, None);
    }
    let _wiz: Vec<Vec3> = s
        .ecs
        .iter()
        .filter(|a| {
            matches!(a.kind, server_core::ActorKind::Wizard)
                && a.faction == server_core::Faction::Pc
        })
        .map(|a| a.tr.pos)
        .collect();
    s.step_authoritative(0.1);
    match s.ecs.get(z) {
        None => {}
        Some(c) => {
            assert!(c.despawn_after.is_some(), "expected despawn timer on death");
        }
    }
}
