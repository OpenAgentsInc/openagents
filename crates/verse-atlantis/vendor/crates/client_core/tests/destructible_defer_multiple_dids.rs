#![allow(clippy::unwrap_used)]
use client_core::replication::ReplicationBuffer;
use net_core::snapshot::{ChunkMeshDelta, DestructibleInstance, SnapshotEncode};

#[test]
fn deltas_are_deferred_per_did_until_instance_arrives() {
    let mut buf = ReplicationBuffer::default();

    // Two different DID deltas, but no instances yet → both deferred
    let d0 = ChunkMeshDelta {
        did: 10,
        chunk: (0, 0, 0),
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    };
    let d1 = ChunkMeshDelta {
        did: 11,
        chunk: (0, 0, 1),
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    };

    for d in [d0, d1] {
        let mut b = Vec::new();
        d.encode(&mut b);
        let mut f = Vec::new();
        net_core::frame::write_msg(&mut f, &b);
        assert!(buf.apply_message(&f));
    }
    assert_eq!(buf.updated_chunks, 0);

    // Send instance for DID 10 → only that DID becomes pending
    let inst10 = DestructibleInstance {
        did: 10,
        world_min: [0.0; 3],
        world_max: [1.0; 3],
    };
    let mut b = Vec::new();
    inst10.encode(&mut b);
    let mut f = Vec::new();
    net_core::frame::write_msg(&mut f, &b);
    assert!(buf.apply_message(&f));
    assert_eq!(buf.updated_chunks, 1);

    // Send instance for DID 11 → now both delivered
    let inst11 = DestructibleInstance {
        did: 11,
        world_min: [0.0; 3],
        world_max: [1.0; 3],
    };
    let mut b = Vec::new();
    inst11.encode(&mut b);
    let mut f = Vec::new();
    net_core::frame::write_msg(&mut f, &b);
    assert!(buf.apply_message(&f));
    assert_eq!(buf.updated_chunks, 2);
}
