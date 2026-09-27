use client_core::replication::ReplicationBuffer;
use net_core::snapshot::{ChunkMeshDelta, DestructibleInstance, SnapshotEncode};

#[test]
fn chunk_delta_outside_instance_aabb_is_rejected() {
    let mut buf = ReplicationBuffer::default();

    // Register an instance near origin
    let inst = DestructibleInstance {
        did: 1,
        world_min: [-2.0, 0.0, -2.0],
        world_max: [2.0, 4.0, 2.0],
    };
    let mut b = Vec::new();
    inst.encode(&mut b);
    let mut f = Vec::new();
    net_core::frame::write_msg(&mut f, &b);
    assert!(buf.apply_message(&f));

    // Fake a delta far away (should be dropped)
    let delta = ChunkMeshDelta {
        did: 1,
        chunk: (0, 0, 0),
        positions: vec![
            [100.0, 100.0, 100.0],
            [101.0, 100.0, 100.0],
            [100.0, 101.0, 100.0],
        ],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    };
    let mut b2 = Vec::new();
    delta.encode(&mut b2);
    let mut f2 = Vec::new();
    net_core::frame::write_msg(&mut f2, &b2);

    // Apply: guard should reject and we keep no pending uploads
    let _ = buf.apply_message(&f2);
    assert!(buf.drain_mesh_updates().is_empty());
}
