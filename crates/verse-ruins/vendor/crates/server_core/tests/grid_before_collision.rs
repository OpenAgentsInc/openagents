use glam::vec3;

// Ensure spatial grid is rebuilt before projectile collision so a single tick can register a hit.
#[test]
fn grid_rebuild_precedes_collision_single_tick_hit() {
    let mut s = server_core::ServerState::new();
    // PC at origin; Wizard target 6m ahead
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    let wiz = s.ecs.spawn(
        server_core::ActorKind::Wizard,
        server_core::Faction::Wizards,
        server_core::Transform {
            pos: vec3(0.0, 0.6, 6.0),
            yaw: 0.0,
            radius: 0.7,
        },
        server_core::Health { hp: 100, max: 100 },
    );
    let hp0 = s.ecs.get(wiz).unwrap().hp.hp;
    // Cast Firebolt once; advance a single tick with dt large enough to cross 6m
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(0.0, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    s.step_authoritative(0.20);
    let hp1 = s.ecs.get(wiz).unwrap().hp.hp;
    assert!(
        hp1 < hp0,
        "Wizard HP did not drop in one tick; grid may not have been rebuilt before collision (hp0={}, hp1={})",
        hp0,
        hp1
    );
}
