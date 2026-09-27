#![allow(clippy::unwrap_used)]

use glam::Vec3;
use server_core as sc;

#[test]
fn fireball_segment_crossing_proxy_enqueues_carve() {
    let mut s = sc::ServerState::new();
    sc::scene_build::add_demo_ruins_destructible(&mut s);

    let inst = s.destruct_instances[0].clone();
    let min = Vec3::from(inst.world_min);
    let max = Vec3::from(inst.world_max);
    let start = Vec3::new(min.x - 1.0, (min.y + max.y) * 0.5, (min.z + max.z) * 0.5);
    let end = Vec3::new(min.x + 1.0, (min.y + max.y) * 0.5, (min.z + max.z) * 0.5);

    // Spawn a projectile entity directly into ECS configured so p0->p1 crosses
    let comps = sc::ecs::Components {
        id: sc::actor::ActorId(0),
        kind: sc::actor::ActorKind::Zombie, // unused for projectile
        faction: sc::actor::Faction::Neutral,
        name: None,
        tr: sc::actor::Transform {
            pos: end,
            yaw: 0.0,
            radius: 0.1,
        },
        hp: sc::actor::Health { hp: 1, max: 1 },
        move_speed: None,
        aggro: None,
        attack: None,
        melee: None,
        projectile: Some(sc::ecs::Projectile {
            kind: sc::ProjKind::Fireball,
            ttl_s: 2.0,
            age_s: 0.5,
        }),
        velocity: Some(sc::ecs::Velocity { v: end - start }),
        owner: None,
        homing: None,
        spellbook: None,
        pool: None,
        cooldowns: None,
        intent_move: None,
        intent_aim: None,
        burning: None,
        slow: None,
        stunned: None,
        despawn_after: None,
    };
    let _pid = s.ecs.spawn_from_components(comps);

    let mut ctx = sc::ecs::schedule::Ctx::default();
    ctx.dt = 1.0;
    sc::ecs::schedule::destructible_from_projectiles_for_test(&mut s, &mut ctx);
    assert!(
        ctx.carves.iter().any(|c| c.did == inst.did),
        "expected carve for crossed proxy"
    );
}

#[test]
fn firebolt_never_enqueues_carve_even_on_crossing() {
    let mut s = sc::ServerState::new();
    sc::scene_build::add_demo_ruins_destructible(&mut s);

    let inst = s.destruct_instances[0].clone();
    let min = Vec3::from(inst.world_min);
    let max = Vec3::from(inst.world_max);
    let start = Vec3::new(min.x - 1.0, (min.y + max.y) * 0.5, (min.z + max.z) * 0.5);
    let end = Vec3::new(min.x + 1.0, (min.y + max.y) * 0.5, (min.z + max.z) * 0.5);

    let comps = sc::ecs::Components {
        id: sc::actor::ActorId(0),
        kind: sc::actor::ActorKind::Zombie,
        faction: sc::actor::Faction::Neutral,
        name: None,
        tr: sc::actor::Transform {
            pos: end,
            yaw: 0.0,
            radius: 0.1,
        },
        hp: sc::actor::Health { hp: 1, max: 1 },
        move_speed: None,
        aggro: None,
        attack: None,
        melee: None,
        projectile: Some(sc::ecs::Projectile {
            kind: sc::ProjKind::Firebolt,
            ttl_s: 2.0,
            age_s: 0.5,
        }),
        velocity: Some(sc::ecs::Velocity { v: end - start }),
        owner: None,
        homing: None,
        spellbook: None,
        pool: None,
        cooldowns: None,
        intent_move: None,
        intent_aim: None,
        burning: None,
        slow: None,
        stunned: None,
        despawn_after: None,
    };
    let _pid = s.ecs.spawn_from_components(comps);

    let mut ctx = sc::ecs::schedule::Ctx::default();
    ctx.dt = 1.0;
    sc::ecs::schedule::destructible_from_projectiles_for_test(&mut s, &mut ctx);
    assert!(ctx.carves.is_empty(), "Firebolt must not enqueue carves");
}
