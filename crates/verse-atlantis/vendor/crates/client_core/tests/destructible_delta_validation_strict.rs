#![allow(clippy::unwrap_used)]

use client_core::replication::ReplicationBuffer;
use net_core::snapshot::{ChunkMeshDelta, DestructibleInstance, SnapshotEncode};

fn frame<T: SnapshotEncode>(msg: &T) -> Vec<u8> {
    let mut b = Vec::new();
    msg.encode(&mut b);
    let mut f = Vec::new();
    net_core::frame::write_msg(&mut f, &b);
    f
}

#[test]
fn indices_out_of_range_are_rejected() {
    let mut buf = ReplicationBuffer::default();

    let inst = DestructibleInstance {
        did: 7,
        world_min: [0.0; 3],
        world_max: [10.0; 3],
    };
    assert!(buf.apply_message(&frame(&inst)));

    // index 3 is out of bounds for 3-position list
    let delta = ChunkMeshDelta {
        did: 7,
        chunk: (0, 0, 0),
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        indices: vec![0, 1, 3],
    };
    let _ = buf.apply_message(&frame(&delta));
    assert!(
        buf.drain_mesh_updates().is_empty(),
        "OOB indices must be rejected"
    );
}

#[test]
fn nan_positions_are_rejected() {
    let mut buf = ReplicationBuffer::default();
    let inst = DestructibleInstance {
        did: 9,
        world_min: [0.0; 3],
        world_max: [10.0; 3],
    };
    assert!(buf.apply_message(&frame(&inst)));

    let delta = ChunkMeshDelta {
        did: 9,
        chunk: (0, 0, 0),
        positions: vec![[f32::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    };
    let _ = buf.apply_message(&frame(&delta));
    assert!(
        buf.drain_mesh_updates().is_empty(),
        "NaN vertex positions must be rejected"
    );
}

#[test]
fn empty_delta_is_accepted_and_drained() {
    let mut buf = ReplicationBuffer::default();
    let inst = DestructibleInstance {
        did: 11,
        world_min: [0.0; 3],
        world_max: [1.0; 3],
    };
    assert!(buf.apply_message(&frame(&inst)));

    // Valid empty delta to clear a chunk
    let delta = ChunkMeshDelta {
        did: 11,
        chunk: (0, 0, 0),
        positions: vec![],
        normals: vec![],
        indices: vec![],
    };
    assert!(buf.apply_message(&frame(&delta)));
    let ups = buf.drain_mesh_updates();
    assert_eq!(ups.len(), 1, "empty delta should still reach upload path");
    assert!(
        buf.drain_mesh_updates().is_empty(),
        "drain empties internal queue"
    );
}

#[test]
fn invalid_instance_does_not_register_did_or_flush_deferred() {
    let mut buf = ReplicationBuffer::default();

    // Delta arrives first and is deferred
    let d = ChunkMeshDelta {
        did: 42,
        chunk: (0, 0, 0),
        positions: vec![[0.0, 0.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]],
        indices: vec![0],
    };
    assert!(buf.apply_message(&frame(&d)));
    assert_eq!(buf.updated_chunks, 0);

    // Malformed instance (max < min) must not register DID or flush
    let bad = DestructibleInstance {
        did: 42,
        world_min: [1.0, 1.0, 1.0],
        world_max: [0.0, 0.0, 0.0],
    };
    let _ = buf.apply_message(&frame(&bad));
    assert_eq!(buf.updated_chunks, 0);

    // Valid instance now flushes exactly one deferred delta
    let good = DestructibleInstance {
        did: 42,
        world_min: [0.0; 3],
        world_max: [1.0; 3],
    };
    assert!(buf.apply_message(&frame(&good)));
    assert_eq!(buf.updated_chunks, 1);
}
