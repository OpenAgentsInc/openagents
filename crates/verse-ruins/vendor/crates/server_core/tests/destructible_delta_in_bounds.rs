#![allow(clippy::unwrap_used)]

#[test]
fn initial_mesh_deltas_are_within_instance_aabb() {
    let mut srv = server_core::ServerState::new();
    server_core::scene_build::add_demo_ruins_destructible(&mut srv);
    // Run one meshing pass to produce initial deltas
    server_core::systems::destructible::destructible_remesh_budgeted(&mut srv);
    let inst = srv.destruct_instances.first().cloned().expect("instance");
    let min = glam::Vec3::from(inst.world_min);
    let max = glam::Vec3::from(inst.world_max);
    let eps = 0.5f32;
    for d in srv.drain_destruct_mesh_deltas() {
        if d.positions.is_empty() {
            continue;
        }
        let mut bmin = glam::Vec3::splat(f32::INFINITY);
        let mut bmax = glam::Vec3::splat(f32::NEG_INFINITY);
        for p in &d.positions {
            let v = glam::vec3(p[0], p[1], p[2]);
            bmin = bmin.min(v);
            bmax = bmax.max(v);
        }
        assert!(bmax.x >= min.x - eps && bmin.x <= max.x + eps);
        assert!(bmax.y >= min.y - eps && bmin.y <= max.y + eps);
        assert!(bmax.z >= min.z - eps && bmin.z <= max.z + eps);
    }
}
