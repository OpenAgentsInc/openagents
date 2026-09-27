use client_core::replication::ReplicationBuffer;
use net_core::snapshot::{HUD_TOAST_VERSION, HudToastMsg, SnapshotEncode};

#[test]
fn hud_toast_not_enough_mana_is_stored() {
    let mut buf = ReplicationBuffer::default();
    let toast = HudToastMsg {
        v: HUD_TOAST_VERSION,
        code: 1,
    };
    let mut b = Vec::new();
    toast.encode(&mut b);
    let mut f = Vec::new();
    net_core::frame::write_msg(&mut f, &b);

    assert!(buf.apply_message(&f));
    assert_eq!(buf.toasts, vec![1u8]);
}
