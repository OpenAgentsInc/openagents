use net_core::snapshot::SnapshotEncode;

#[test]
fn client_parses_hud_status() {
    use net_core::snapshot::{HUD_STATUS_VERSION, HudStatusMsg};
    let hud = HudStatusMsg {
        v: HUD_STATUS_VERSION,
        mana: 11,
        mana_max: 20,
        gcd_ms: 250,
        spell_cds: vec![(0, 0), (1, 1500), (2, 500)],
        burning_ms: 900,
        slow_ms: 400,
        stunned_ms: 0,
    };
    let mut buf = Vec::new();
    hud.encode(&mut buf);
    let mut framed = Vec::new();
    net_core::frame::write_msg(&mut framed, &buf);

    let mut repl = client_core::replication::ReplicationBuffer::default();
    assert!(repl.apply_message(&framed));
    assert_eq!(repl.hud.mana, 11);
    assert_eq!(repl.hud.mana_max, 20);
    assert_eq!(repl.hud.gcd_ms, 250);
    assert_eq!(repl.hud.spell_cds[1], 1500);
    assert_eq!(repl.hud.burning_ms, 900);
    assert_eq!(repl.hud.slow_ms, 400);
    assert_eq!(repl.hud.stunned_ms, 0);
}
