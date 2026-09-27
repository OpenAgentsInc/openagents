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
fn wizard_is_pc_flag_set_from_faction() {
    let mut buf = ReplicationBuffer::default();
    let pc = ActorRep {
        id: 1,
        kind: 0,    // Wizard
        faction: 0, // Pc
        archetype_id: 1,
        name_id: 1,
        unique: 0,
        pos: [0.0, 0.6, 0.0],
        yaw: 0.0,
        radius: 0.7,
        hp: 100,
        max: 100,
        alive: true,
    };
    let npc = ActorRep {
        id: 2,
        kind: 0,    // Wizard
        faction: 1, // Wizards (NPC)
        archetype_id: 1,
        name_id: 0,
        unique: 0,
        pos: [5.0, 0.6, 0.0],
        yaw: 0.0,
        radius: 0.7,
        hp: 100,
        max: 100,
        alive: true,
    };
    let delta = ActorSnapshotDelta {
        v: 4,
        tick: 1,
        baseline: 0,
        spawns: vec![pc, npc],
        updates: vec![],
        removals: vec![],
        projectiles: vec![ProjectileRep {
            id: 99,
            kind: 0,
            pos: [0.0, 0.0, 0.0],
            vel: [0.0, 0.0, 1.0],
        }],
        hits: vec![],
    };
    assert!(buf.apply_message(&frame(&delta)));
    assert_eq!(buf.wizards.len(), 2);
    let pcw = buf.wizards.iter().find(|w| w.id == 1).unwrap();
    let npcw = buf.wizards.iter().find(|w| w.id == 2).unwrap();
    assert!(pcw.is_pc);
    assert!(!npcw.is_pc);
}
