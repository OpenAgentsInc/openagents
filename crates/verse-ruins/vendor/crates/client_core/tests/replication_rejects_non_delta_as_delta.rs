use client_core::replication::ReplicationBuffer;
use net_core::snapshot::{HUD_TOAST_VERSION, HudToastMsg, SnapshotEncode};

#[test]
fn replication_does_not_treat_unrelated_frames_as_chunk_delta() {
    let mut buf = ReplicationBuffer::default();
    // Build a HUD toast frame (not a delta)
    let mut b = Vec::new();
    HudToastMsg {
        v: HUD_TOAST_VERSION,
        code: 2,
    }
    .encode(&mut b);
    let mut f = Vec::new();
    net_core::frame::write_msg(&mut f, &b);

    assert!(buf.apply_message(&f), "toast should be accepted by handler");
    assert!(buf.drain_mesh_updates().is_empty());
    assert_eq!(buf.updated_chunks, 0);
}
