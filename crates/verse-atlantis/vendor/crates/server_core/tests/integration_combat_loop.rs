// crates/server_core/tests/integration_combat_loop.rs
use glam::{Vec3, vec3};

fn count_undead_alive(s: &server_core::ServerState) -> usize {
    s.ecs
        .iter()
        .filter(|a| {
            matches!(
                a.kind,
                server_core::ActorKind::Zombie | server_core::ActorKind::Boss
            )
        })
        .filter(|a| a.faction == server_core::Faction::Undead)
        .filter(|a| a.hp.alive())
        .count()
}

fn any_undead_damaged(s: &server_core::ServerState) -> bool {
    s.ecs
        .iter()
        .any(|a| a.faction == server_core::Faction::Undead && a.hp.hp < a.hp.max)
}

fn any_caster_damaged(s: &server_core::ServerState) -> bool {
    s.ecs
        .iter()
        .any(|a| matches!(a.kind, server_core::ActorKind::Wizard) && a.hp.hp < a.hp.max)
}

fn total_undead_hp(s: &server_core::ServerState) -> i32 {
    s.ecs
        .iter()
        .filter(|a| a.faction == server_core::Faction::Undead)
        .map(|a| a.hp.hp)
        .sum()
}

fn any_actor_moved(s0: &[(u32, Vec3)], s1: &server_core::ServerState) -> bool {
    // did any actor move a meaningful amount? (for anim state)
    s0.iter().any(|(id, p0)| {
        if let Some(a) = s1.ecs.iter().find(|a| a.id.0 == *id) {
            (a.tr.pos - *p0).length() > 0.05
        } else {
            false
        }
    })
}

#[test]
fn integration_combat_loop_pc_and_casters_vs_undead() {
    let mut s = server_core::ServerState::new();

    // 1) Spawn PC at origin (server-authoritative)
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));

    // 2) Spawn a small circle of NPC wizard casters near the center so undead don't tunnel only the PC
    let wiz_count = 3usize;
    let wiz_r = 8.0f32;
    for i in 0..wiz_count {
        let a = (i as f32) / (wiz_count as f32) * std::f32::consts::TAU;
        let p = vec3(wiz_r * a.cos(), 0.6, wiz_r * a.sin());
        let _ = s.spawn_wizard_npc(p);
    }

    // 3) Spawn rings of undead (two rings, moderate HP so we can see deaths and damage)
    s.ring_spawn(12, 15.0, 30);
    s.ring_spawn(12, 30.0, 40);

    // Baselines
    let undead_alive_0 = count_undead_alive(&s);
    let undead_hp_0 = total_undead_hp(&s);
    // Save initial positions to detect motion (anim driver)
    let snapshot_positions: Vec<(u32, Vec3)> = s.ecs.iter().map(|a| (a.id.0, a.tr.pos)).collect();

    // 4) Run authoritative ECS for ~5s (100 * 50ms)
    let dt = 0.05f32;
    for _ in 0..100 {
        s.step_authoritative(dt); // no mirroring; intents/AI drive motion
    }

    // Post-conditions
    let undead_alive_1 = count_undead_alive(&s);
    let undead_hp_1 = total_undead_hp(&s);

    // Movement happened (positions changed) — animation can key off this
    assert!(
        any_actor_moved(&snapshot_positions, &s),
        "no actor moved enough; animation will appear frozen"
    );

    // Undead should have taken damage (sum HP lower) and/or deaths reduced alive count
    assert!(
        undead_hp_1 < undead_hp_0 || undead_alive_1 < undead_alive_0 || any_undead_damaged(&s),
        "no undead took damage or died (alive0={}, alive1={}, hp0={}, hp1={})",
        undead_alive_0,
        undead_alive_1,
        undead_hp_0,
        undead_hp_1
    );

    // Wizards should also take some damage over the skirmish (not only one-sided)
    assert!(
        any_caster_damaged(&s),
        "no caster took any damage; combat looks one-sided / targeting bug likely"
    );
}
