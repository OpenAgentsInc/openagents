use server_core::ServerState;

#[test]
fn spawn_is_unique_and_status_exists() {
    let mut s = ServerState::new();
    let id1 = s
        .spawn_nivita_unique(glam::vec3(0.0, 0.6, 10.0))
        .expect("spawn");
    let id2 = s
        .spawn_nivita_unique(glam::vec3(5.0, 0.6, 15.0))
        .expect("spawn again");
    assert_eq!(id1.0, id2.0, "must return same id for unique boss");
    let st = s.nivita_status().expect("status");
    assert!(st.name.to_lowercase().contains("nivita"));
}
