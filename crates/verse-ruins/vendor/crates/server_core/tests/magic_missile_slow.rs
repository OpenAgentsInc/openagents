use glam::Vec3;
use server_core::SpellId;

#[test]
fn magic_missile_applies_slow_on_hit() {
    let mut s = server_core::ServerState::new();
    s.sync_wizards(&[Vec3::new(0.0, 0.6, 0.0)]);
    let z = s.spawn_undead(Vec3::new(0.0, 0.6, 8.0), 0.9, 40);
    s.enqueue_cast(
        Vec3::new(0.0, 0.6, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        SpellId::MagicMissile,
    );
    for _ in 0..15 {
        let _wiz: Vec<Vec3> = s
            .ecs
            .iter()
            .filter(|a| {
                matches!(a.kind, server_core::ActorKind::Wizard)
                    && a.faction == server_core::Faction::Pc
            })
            .map(|a| a.tr.pos)
            .collect();
        s.step_authoritative(0.1);
    }
    if let Some(a) = s.ecs.get(z)
        && a.hp.alive()
    {
        assert!(a.slow.is_some(), "expected Slow applied to intended target");
    }
    let any_slow = s.ecs.iter().any(|c| c.slow.is_some());
    assert!(any_slow, "expected at least one Slow after MM volley");
}
