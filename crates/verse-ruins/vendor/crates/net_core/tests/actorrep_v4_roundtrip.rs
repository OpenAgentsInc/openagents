use net_core::snapshot::{ActorRep, ActorSnapshotDelta, SnapshotDecode, SnapshotEncode};

#[test]
fn actorrep_v4_roundtrip() {
    let spawn = ActorRep {
        id: 42,
        kind: 7,
        faction: 3,
        archetype_id: 123,
        name_id: 55,
        unique: 1,
        pos: [1.0, 2.0, 3.0],
        yaw: 0.75,
        radius: 0.8,
        hp: 9,
        max: 10,
        alive: true,
    };
    let delta = ActorSnapshotDelta {
        v: 4,
        tick: 9,
        baseline: 8,
        spawns: vec![spawn],
        updates: vec![],
        removals: vec![],
        projectiles: vec![],
        hits: vec![],
    };
    let mut buf = Vec::new();
    delta.encode(&mut buf);
    let mut slice: &[u8] = &buf;
    let d2 = ActorSnapshotDelta::decode(&mut slice).expect("decode v4");
    assert_eq!(d2.v, 4);
    assert_eq!(d2.tick, 9);
    assert_eq!(d2.baseline, 8);
    assert_eq!(d2.spawns.len(), 1);
    let a = &d2.spawns[0];
    assert_eq!(a.id, 42);
    assert_eq!(a.faction, 3);
    assert_eq!(a.archetype_id, 123);
    assert_eq!(a.name_id, 55);
    assert_eq!(a.unique, 1);
}
