use net_core::snapshot::{
    ActorSnapshotDelta, HitFx, ProjectileRep, SnapshotDecode, SnapshotEncode,
};

#[test]
fn hitfx_roundtrip_in_actor_delta() {
    let delta = ActorSnapshotDelta {
        v: 4,
        tick: 100,
        baseline: 90,
        spawns: vec![],
        updates: vec![],
        removals: vec![],
        projectiles: vec![ProjectileRep {
            id: 1,
            kind: 0,
            pos: [0.0, 0.6, 0.0],
            vel: [1.0, 0.0, 0.0],
        }],
        hits: vec![HitFx {
            kind: 2,
            pos: [3.0, 0.6, -1.0],
        }],
    };
    let mut buf = Vec::new();
    delta.encode(&mut buf);
    let mut slice: &[u8] = &buf;
    let d2 = ActorSnapshotDelta::decode(&mut slice).expect("decode");
    assert_eq!(d2.tick, 100);
    assert_eq!(d2.baseline, 90);
    assert_eq!(d2.projectiles.len(), 1);
    assert_eq!(d2.hits.len(), 1);
    assert_eq!(d2.hits[0].kind, 2);
    assert!((d2.hits[0].pos[0] - 3.0).abs() < 1.0e-6);
}
