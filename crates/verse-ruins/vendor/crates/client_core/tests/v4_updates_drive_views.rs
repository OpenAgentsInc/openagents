use client_core::replication::ReplicationBuffer;
use net_core::snapshot::{ActorDeltaRec, ActorRep, ActorSnapshotDelta, SnapshotEncode};

#[test]
fn v4_updates_drive_wizard_view() {
    // Spawn one wizard (id=10, hp=100)
    let spawn = ActorRep {
        id: 10,
        kind: 0,
        faction: 1,
        archetype_id: 1,
        name_id: 0,
        unique: 0,
        pos: [0.0, 0.6, 0.0],
        yaw: 0.0,
        radius: 0.7,
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
    let mut buf = Vec::new();
    delta0.encode(&mut buf);
    let mut repl = ReplicationBuffer::default();
    assert!(repl.apply_message(&buf));
    assert_eq!(repl.wizards.len(), 1);
    assert_eq!(repl.wizards[0].hp, 100);

    // Apply HP-only update (flags=HP)
    let upd = ActorDeltaRec {
        id: 10,
        flags: 4,
        qpos: [0; 3],
        qyaw: 0,
        hp: 90,
        alive: 0,
    };
    let delta1 = ActorSnapshotDelta {
        v: 4,
        tick: 2,
        baseline: 1,
        spawns: vec![],
        updates: vec![upd],
        removals: vec![],
        projectiles: vec![],
        hits: vec![],
    };
    let mut buf2 = Vec::new();
    delta1.encode(&mut buf2);
    assert!(repl.apply_message(&buf2));
    assert_eq!(repl.actors[0].hp, 90);
    assert_eq!(
        repl.wizards[0].hp, 90,
        "wizard view must reflect actor HP update"
    );

    // Removal
    let delta2 = ActorSnapshotDelta {
        v: 4,
        tick: 3,
        baseline: 2,
        spawns: vec![],
        updates: vec![],
        removals: vec![10],
        projectiles: vec![],
        hits: vec![],
    };
    let mut buf3 = Vec::new();
    delta2.encode(&mut buf3);
    assert!(repl.apply_message(&buf3));
    assert!(repl.actors.is_empty());
    assert!(repl.wizards.is_empty());
}
