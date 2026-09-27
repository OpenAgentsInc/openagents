use client_core::replication::ReplicationBuffer;
use client_core::upload::{ChunkMeshEntry, MeshUpload};
use net_core::{
    channel,
    snapshot::{ActorRep, ActorSnapshotDelta, ChunkMeshDelta, ProjectileRep, SnapshotEncode},
};

struct DummyUploader {
    uploads: usize,
    last: Option<(u64, (u32, u32, u32), ChunkMeshEntry)>,
}
impl MeshUpload for DummyUploader {
    fn upload_chunk_mesh(&mut self, did: u64, chunk: (u32, u32, u32), mesh: &ChunkMeshEntry) {
        self.uploads += 1;
        self.last = Some((did, chunk, mesh.clone()));
    }
    fn remove_chunk_mesh(&mut self, _did: u64, _chunk: (u32, u32, u32)) {}
}

#[test]
fn local_loop_sends_and_applies_chunk_mesh() {
    let (tx, rx) = channel::channel();
    // Server encodes a minimal instance then a small delta
    let inst = net_core::snapshot::DestructibleInstance {
        did: 7,
        world_min: [-1.0, 0.0, -1.0],
        world_max: [1.0, 2.0, 1.0],
    };
    let mut inst_buf = Vec::new();
    inst.encode(&mut inst_buf);
    assert!(tx.try_send(inst_buf));
    // Delta for the same DID
    let delta = ChunkMeshDelta {
        did: 7,
        chunk: (1, 2, 3),
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    };
    let mut buf = Vec::new();
    delta.encode(&mut buf);
    assert!(tx.try_send(buf));

    // Client drains and applies
    let mut repl = ReplicationBuffer::default();
    for b in rx.drain() {
        let _ = repl.apply_message(&b);
    }
    assert_eq!(repl.updated_chunks, 1);

    // Renderer (or host) uploads
    let mut up = DummyUploader {
        uploads: 0,
        last: None,
    };
    for (did, chunk, entry) in repl.drain_mesh_updates() {
        up.upload_chunk_mesh(did, chunk, &entry);
    }
    assert_eq!(up.uploads, 1);
    let last = up.last.unwrap();
    assert_eq!(last.0, 7);
    assert_eq!(last.1, (1, 2, 3));
    assert_eq!(last.2.indices, vec![0, 1, 2]);
}

#[test]
fn v3_delta_populates_projectiles() {
    let mut repl = ReplicationBuffer::default();
    // Build a minimal v3 delta with one projectile
    let delta = ActorSnapshotDelta {
        v: 4,
        tick: 1,
        baseline: 0,
        spawns: vec![ActorRep {
            id: 1,
            kind: 0,
            faction: 0,
            archetype_id: 1,
            name_id: 0,
            unique: 0,
            pos: [0.0, 0.0, 0.0],
            yaw: 0.0,
            radius: 0.7,
            hp: 100,
            max: 100,
            alive: true,
        }],
        updates: vec![],
        removals: vec![],
        projectiles: vec![ProjectileRep {
            id: 99,
            kind: 1,
            pos: [1.0, 0.5, 2.0],
            vel: [0.0, 0.0, 1.0],
        }],
        hits: vec![],
    };
    let mut buf = Vec::new();
    delta.encode(&mut buf);
    // Frame and apply
    let mut framed = Vec::new();
    net_core::frame::write_msg(&mut framed, &buf);
    assert!(repl.apply_message(&framed));
    assert_eq!(repl.projectiles.len(), 1);
    assert_eq!(repl.projectiles[0].id, 99);
}
