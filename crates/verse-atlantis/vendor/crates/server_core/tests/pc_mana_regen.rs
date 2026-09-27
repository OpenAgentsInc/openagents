#[test]
fn pc_mana_regen_is_one_per_second() {
    let mut s = server_core::ServerState::new();
    let pc = s.spawn_pc_at(glam::vec3(0.0, 0.6, 0.0));
    {
        let a = s.ecs.get_mut(pc).unwrap();
        let pool = a.pool.as_mut().unwrap();
        pool.mana = 10;
    }
    for _ in 0..60 {
        s.step_authoritative(1.0 / 60.0);
    }
    let mana = s.ecs.get(pc).unwrap().pool.as_ref().unwrap().mana;
    assert_eq!(mana, 11, "PC mana should regen by ~1 over one second");
}
