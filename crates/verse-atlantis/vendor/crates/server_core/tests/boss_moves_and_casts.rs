use glam::vec3;

#[test]
fn death_knight_moves_and_casts_projectiles() {
    let mut s = server_core::ServerState::new();
    // Spawn PC at origin and a ring of wizards so DK has targets
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    let wiz = s.spawn_wizard_npc(vec3(6.0, 0.6, 0.0));
    let dk = s.spawn_death_knight(vec3(25.0, 0.6, -15.0));
    let d0 = {
        let dkpos = s.ecs.get(dk).unwrap().tr.pos;
        let wzpos = s.ecs.get(wiz).unwrap().tr.pos;
        (dkpos - wzpos).length()
    };
    // Step a few seconds total; DK should move closer and cast at least once
    let mut saw_proj = false;
    for _ in 0..60 {
        s.step_authoritative(0.1);
        if s.ecs.iter().any(|c| c.projectile.is_some()) {
            saw_proj = true;
        }
    }
    let d1 = {
        let dkpos = s.ecs.get(dk).unwrap().tr.pos;
        let wzpos = s.ecs.get(wiz).unwrap().tr.pos;
        (dkpos - wzpos).length()
    };
    assert!(d1 < d0, "DK did not move closer: d0={:.2} d1={:.2}", d0, d1);
    assert!(saw_proj, "DK did not cast any projectile in 6s window");
}
