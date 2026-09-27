use server_core::ServerState;

#[test]
fn boot_with_zone_spawns_demo_for_wizard_woods() {
    let mut s = ServerState::new();
    let spawned = server_core::zones::boot_with_zone(&mut s, "wizard_woods");
    assert!(spawned, "wizard_woods should spawn demo content");
    // Expect at least DK + some wizards in ECS
    assert!(s.ecs.len() > 0, "server ECS should have actors after boot");
    assert!(s.nivita_actor_id.is_some(), "unique boss should be spawned");
    assert!(
        !s.destruct_instances.is_empty(),
        "demo destructible instance should be registered"
    );
}

#[test]
fn boot_with_zone_no_spawns_for_campaign_builder() {
    let mut s = ServerState::new();
    let spawned = server_core::zones::boot_with_zone(&mut s, "campaign_builder");
    assert!(
        !spawned,
        "campaign_builder should not spawn any demo content"
    );
    assert_eq!(s.ecs.len(), 0, "no actors spawned");
    assert!(s.nivita_actor_id.is_none(), "no unique boss spawned");
    assert!(
        s.destruct_instances.is_empty(),
        "no destructible instances should be registered"
    );
}

#[test]
fn boot_with_zone_unknown_slug_is_noop() {
    let mut s = ServerState::new();
    let spawned = server_core::zones::boot_with_zone(&mut s, "some_unknown_slug");
    assert!(!spawned, "unknown zones must not spawn anything");
    assert!(s.ecs.is_empty());
    assert!(s.destruct_instances.is_empty());
    assert!(s.nivita_actor_id.is_none());
}
