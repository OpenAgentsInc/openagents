use glam::Vec3;

#[test]
fn projectile_broadphase_uses_grid_candidates() {
    let mut s = server_core::ServerState::new();
    // Spawn PC and a bunch of undead spread across the map
    let _pc = s.spawn_pc_at(Vec3::new(0.0, 0.6, -5.0));
    for i in 0..200 {
        let x = (i as f32 % 20.0) * 3.5 - 35.0;
        let z = (i as f32 / 20.0).floor() * 3.5 + 5.0;
        let _ = s.spawn_undead(Vec3::new(x, 0.6, z), 0.9, 20);
    }
    // Query a short segment in the center with small pad via public helper
    let a = glam::Vec2::new(-2.0, 5.0);
    let b = glam::Vec2::new(2.0, 7.0);
    let cand_len = s.spatial_candidates_for_segment(a, b, 2.0);
    // Should be far fewer than total actors (PC + 200 undead)
    assert!(
        cand_len < 80,
        "broad-phase candidate set should be much smaller than total actors"
    );
}
