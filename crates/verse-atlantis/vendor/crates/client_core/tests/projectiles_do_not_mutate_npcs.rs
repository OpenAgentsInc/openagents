#![allow(clippy::unwrap_used)]

use client_core::replication::ReplicationBuffer;
use net_core::snapshot::{ActorRep, ActorSnapshotDelta, ProjectileRep, SnapshotEncode};

fn frame<T: SnapshotEncode>(msg: &T) -> Vec<u8> {
    let mut b = Vec::new();
    msg.encode(&mut b);
    let mut f = Vec::new();
    net_core::frame::write_msg(&mut f, &b);
    f
}

#[test]
fn projectile_id_equal_to_npc_id_does_not_change_npc_view() {
    let mut buf = ReplicationBuffer::default();

    // Spawn one NPC with id 5
    let spawn = ActorRep {
        id: 5,
        kind: 1,
        faction: 2,
        archetype_id: 2,
        name_id: 0,
        unique: 0,
        pos: [0.0, 0.6, 0.0],
        yaw: 0.0,
        radius: 0.9,
        hp: 100,
        max: 100,
        alive: true,
    };
    let delta0 = ActorSnapshotDelta {
        v: 4,
        tick: 1,
        baseline: 0,
        spawns: vec![spawn],
        updates: vec![],
        removals: vec![],
        projectiles: vec![],
        hits: vec![],
    };
    assert!(buf.apply_message(&frame(&delta0)));
    assert_eq!(buf.npcs.len(), 1);

    // Next tick: projectile with same id=5 must not mutate NPC views
    let delta1 = ActorSnapshotDelta {
        v: 4,
        tick: 2,
        baseline: 1,
        spawns: vec![],
        updates: vec![],
        removals: vec![],
        projectiles: vec![ProjectileRep {
            id: 5,
            kind: 2,
            pos: [10.0, 1.0, 0.0],
            vel: [0.0, 0.0, 1.0],
        }],
        hits: vec![],
    };
    assert!(buf.apply_message(&frame(&delta1)));
    assert_eq!(
        buf.npcs.len(),
        1,
        "projectiles must not overwrite NPC views"
    );
}
