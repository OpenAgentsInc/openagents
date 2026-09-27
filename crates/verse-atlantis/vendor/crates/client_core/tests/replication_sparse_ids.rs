use net_core::snapshot::{ActorRep, ActorSnapshotDelta, SnapshotEncode};

#[test]
fn apply_actor_delta_with_sparse_id_does_not_panic() {
    let delta = ActorSnapshotDelta {
        v: 4,
        tick: 1,
        baseline: 0,
        spawns: vec![ActorRep {
            id: 100,
            kind: 1,
            faction: 2,
            archetype_id: 2,
            name_id: 0,
            unique: 0,
            pos: [1.0, 0.6, 2.0],
            yaw: 0.0,
            radius: 0.9,
            hp: 30,
            max: 30,
            alive: true,
        }],
        updates: vec![],
        removals: vec![],
        projectiles: vec![],
        hits: vec![],
    };
    let mut buf = Vec::new();
    delta.encode(&mut buf);
    let mut framed = Vec::new();
    net_core::frame::write_msg(&mut framed, &buf);

    let mut repl = client_core::replication::ReplicationBuffer::default();
    assert!(repl.apply_message(&framed));
    // At least one actor present, with matching id, no panic occurred
    assert!(repl.actors.iter().any(|a| a.id == 100));
}
