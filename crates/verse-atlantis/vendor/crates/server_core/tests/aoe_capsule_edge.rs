#![allow(clippy::unwrap_used)]

use server_core as sc;

#[test]
fn aoe_at_edge_with_min_capsule_still_hits() {
    let mut s = sc::ServerState::new();
    let z = s.spawn_undead(glam::vec3(3.0, 0.6, 0.0), 0.9, 10);
    let mut ctx = sc::ecs::schedule::Ctx::default();
    ctx.boom.push(sc::ecs::schedule::ExplodeEvent {
        center_xz: glam::vec2(0.0, 0.0),
        r2: 3.0 * 3.0,
        src: None,
    });
    sc::ecs::schedule::aoe_apply_explosions_for_test(&mut s, &mut ctx);
    sc::ecs::schedule::apply_damage_to_ecs_for_test(&mut s, &mut ctx);
    let after = s.ecs.get(z).unwrap();
    assert!(after.hp.hp < after.hp.max);
}
