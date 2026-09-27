use server_core::ServerState;

#[test]
fn status_values_match_config() {
    let mut s = ServerState::new();
    let _ = s
        .spawn_nivita_unique(glam::vec3(0.0, 0.6, 10.0))
        .expect("spawn");
    let st = s.nivita_status().expect("status");
    let cfg = data_runtime::configs::npc_unique::load_nivita().expect("cfg");
    let exp_ac = i32::from(cfg.defenses.ac);
    assert_eq!(st.ac, exp_ac);
    let mid = (cfg.hp_range.0 + cfg.hp_range.1) / 2;
    assert_eq!(st.max, mid);
    assert_eq!(st.hp, mid);
    assert!(st.name.contains(&cfg.name));
}
