use glam::Vec3;

#[test]
fn undead_never_spawns_inside_pc_bubble() {
    let mut s = server_core::ServerState::new();
    // Place PC at origin
    s.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0)]);
    // Attempt to spawn very close to PC; spawn_undead must push it out
    let z = s.spawn_undead(Vec3::new(0.5, 0.6, 0.5), 0.9, 10);
    let pcpos = s.ecs.get(s.pc_actor.unwrap()).unwrap().tr.pos;
    let upos = s.ecs.get(z).unwrap().tr.pos;
    let dx = upos.x - pcpos.x;
    let dz = upos.z - pcpos.z;
    // SAFE_SPAWN_RADIUS_M is 10.0 in server_core; match that expectation here
    assert!(
        dx * dx + dz * dz >= 10.0 * 10.0,
        "spawn not pushed out of PC bubble"
    );
}
