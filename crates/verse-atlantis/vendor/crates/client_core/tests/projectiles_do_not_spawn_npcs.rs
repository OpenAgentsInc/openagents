use client_core::replication::ReplicationBuffer;
use net_core::snapshot::{ActorRep, ActorSnapshotDelta, ProjectileRep, SnapshotEncode};

#[test]
fn projectiles_do_not_create_npc_views() {
    let mut buf = ReplicationBuffer::default();
    // Start with one NPC spawn so we can detect unwanted growth
    let spawn = ActorRep {
        id: 1,
        kind: 1, // NPC archetype
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
    let mut b0 = Vec::new();
    delta0.encode(&mut b0);
    let mut f0 = Vec::new();
    net_core::frame::write_msg(&mut f0, &b0);
    assert!(buf.apply_message(&f0));
    let npc_len0 = buf.npcs.len();
    assert_eq!(npc_len0, 1);

    // Now send a delta with only projectiles; npc count must not grow
    let delta1 = ActorSnapshotDelta {
        v: 4,
        tick: 2,
        baseline: 1,
        spawns: vec![],
        updates: vec![],
        removals: vec![],
        projectiles: vec![ProjectileRep {
            id: 77,
            kind: 2, // MagicMissile visual
            pos: [10.0, 1.2, -5.0],
            vel: [0.0, 0.0, 1.0],
        }],
        hits: vec![],
    };
    let mut b1 = Vec::new();
    delta1.encode(&mut b1);
    let mut f1 = Vec::new();
    net_core::frame::write_msg(&mut f1, &b1);
    assert!(buf.apply_message(&f1));
    assert_eq!(
        buf.npcs.len(),
        npc_len0,
        "projectiles must not create npc views"
    );
}
