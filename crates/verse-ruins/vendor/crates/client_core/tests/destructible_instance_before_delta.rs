#![allow(clippy::unwrap_used)]

use client_core::replication::ReplicationBuffer;
use client_core::upload::ChunkMeshEntry;

#[test]
fn delta_before_instance_is_deferred_until_instance_arrives() {
    use net_core::snapshot::{ChunkMeshDelta, DestructibleInstance, SnapshotEncode};
    // Build a mesh delta for DID=42
    let delta = ChunkMeshDelta {
        did: 42,
        chunk: (1, 2, 3),
        positions: vec![[0.0, 0.0, 0.0]; 3],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    };
    let mut buf_delta = Vec::new();
    delta.encode(&mut buf_delta);
    // Build the instance message for the same DID
    let inst = DestructibleInstance {
        did: 42,
        world_min: [-1.0, 0.0, -1.0],
        world_max: [1.0, 2.0, 1.0],
    };
    let mut buf_inst = Vec::new();
    inst.encode(&mut buf_inst);

    let mut repl = ReplicationBuffer::default();
    // Apply delta first — should be deferred
    let _ = repl.apply_message(&buf_delta);
    assert_eq!(
        repl.drain_mesh_updates().len(),
        0,
        "delta must be deferred until instance arrives"
    );
    // Apply instance — should register DID
    let _ = repl.apply_message(&buf_inst);
    // Re-apply delta; now it should surface
    let _ = repl.apply_message(&buf_delta);
    let drained = repl.drain_mesh_updates();
    assert_eq!(drained.len(), 1);
    let (did, chunk, entry): (u64, (u32, u32, u32), ChunkMeshEntry) =
        drained.into_iter().next().unwrap();
    assert_eq!(did, 42);
    assert_eq!(chunk, (1, 2, 3));
    assert_eq!(entry.indices, vec![0, 1, 2]);
}
