use glam::Vec3;

#[test]
fn enqueue_cast_spawns_projectiles_in_ecs() {
    let mut s = server_core::ServerState::new();
    // Mirror PC position so we have a caster
    s.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0)]);
    // Enqueue a simple Firebolt cast
    s.enqueue_cast(
        Vec3::new(0.0, 0.6, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        server_core::SpellId::Firebolt,
    );
    // Step once; schedule should translate casts to ECS projectiles
    s.step_authoritative(0.016);
    // Assert at least one ECS entity with projectile+velocity exists
    let mut found = false;
    for c in s.ecs.iter() {
        if c.projectile.is_some() && c.velocity.is_some() {
            found = true;
            break;
        }
    }
    assert!(found, "no ECS projectile spawned from cast");
}
