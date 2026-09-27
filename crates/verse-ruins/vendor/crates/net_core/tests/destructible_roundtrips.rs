use net_core::snapshot::{ChunkMeshDelta, DestructibleInstance, SnapshotDecode, SnapshotEncode};

#[test]
fn instance_roundtrip() {
    let inst = DestructibleInstance {
        did: 42,
        world_min: [-1.0, 0.0, -1.0],
        world_max: [1.0, 2.0, 1.0],
    };
    let mut b = Vec::new();
    inst.encode(&mut b);
    let mut s: &[u8] = &b;
    let dec = DestructibleInstance::decode(&mut s).unwrap();
    assert_eq!(dec.did, 42);
    assert!((dec.world_max[1] - 2.0).abs() < 1e-6);
}

#[test]
fn chunk_mesh_delta_roundtrip() {
    let d = ChunkMeshDelta {
        did: 7,
        chunk: (5, 3, 1),
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]; 2],
        indices: vec![0, 1, 1],
    };
    let mut b = Vec::new();
    d.encode(&mut b);
    let mut s: &[u8] = &b;
    let dec = ChunkMeshDelta::decode(&mut s).unwrap();
    assert_eq!(dec.did, 7);
    assert_eq!(dec.chunk, (5, 3, 1));
    assert_eq!(dec.indices.len(), 3);
}
