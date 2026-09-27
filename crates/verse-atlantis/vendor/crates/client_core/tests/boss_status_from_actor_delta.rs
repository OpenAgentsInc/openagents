use client_core::replication::ReplicationBuffer;
use net_core::snapshot::{ActorRep, ActorSnapshotDelta, SnapshotEncode};

#[test]
fn boss_status_populated_from_actor_delta() {
    let mut buf = ReplicationBuffer::default();
    // Spawn one boss actor with HP
    let boss = ActorRep {
        id: 99,
        kind: 2,    // Boss
        faction: 2, // Undead
        archetype_id: 3,
        name_id: 1,
        unique: 1,
        pos: [10.0, 0.6, -5.0],
        yaw: 0.0,
        radius: 1.0,
        hp: 220,
        max: 250,
        alive: true,
    };
    let delta = ActorSnapshotDelta {
        v: 4,
        tick: 1,
        baseline: 0,
        spawns: vec![boss],
        updates: vec![],
        removals: vec![],
        projectiles: vec![],
        hits: vec![],
    };
    let mut payload = Vec::new();
    delta.encode(&mut payload);
    let mut framed = Vec::with_capacity(payload.len() + 8);
    net_core::frame::write_msg(&mut framed, &payload);
    assert!(buf.apply_message(&framed));
    let bs = buf.boss_status.expect("boss status");
    assert_eq!(bs.hp, 220);
    assert_eq!(bs.max, 250);
    assert!((bs.pos.x - 10.0).abs() < 1e-3);
}
