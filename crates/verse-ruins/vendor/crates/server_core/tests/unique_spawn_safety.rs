use glam::Vec3;

#[test]
fn unique_spawn_respects_pc_bubble() {
    let mut s = server_core::ServerState::new();
    // Place PC at origin
    s.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0)]);
    // Ask to spawn boss at origin; helper should push out to safe radius
    let id = s.spawn_nivita_unique(Vec3::new(0.0, 0.6, 0.0)).unwrap();
    let pcpos = s.ecs.get(s.pc_actor.unwrap()).unwrap().tr.pos;
    let bpos = s.ecs.get(id).unwrap().tr.pos;
    let dx = bpos.x - pcpos.x;
    let dz = bpos.z - pcpos.z;
    assert!(
        dx * dx + dz * dz >= 10.0 * 10.0,
        "boss not pushed out of PC bubble"
    );
}
