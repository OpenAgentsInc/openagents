use glam::vec3;

#[test]
fn undead_separation_applies() {
    let mut s = server_core::ServerState::new();
    // Two undead spawned nearly at the same spot
    let u1 = s.spawn_undead(vec3(0.0, 0.6, 0.0), 0.9, 20);
    let u2 = s.spawn_undead(vec3(0.05, 0.6, 0.0), 0.9, 20);
    // Step a few frames to allow separation pass
    for _ in 0..5 {
        s.step_authoritative(0.05);
    }
    let p1 = s.ecs.get(u1).unwrap().tr.pos;
    let p2 = s.ecs.get(u2).unwrap().tr.pos;
    let dist = (p2 - p1).length();
    assert!(dist >= 1.7, "undead should be pushed apart; dist={}", dist);
}
