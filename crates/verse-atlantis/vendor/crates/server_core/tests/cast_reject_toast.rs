use server_core as sc;

#[test]
fn magic_missile_rejects_below_cost_and_emits_toast() {
    let mut s = sc::ServerState::new();
    let _pc = s.spawn_pc_at(glam::vec3(0.0, 0.6, 0.0));

    // Drain mana to 0
    {
        let id = s.pc_actor.unwrap();
        if let Some(pc) = s.ecs.get_mut(id) {
            if let Some(p) = pc.pool.as_mut() {
                p.mana = 0;
            }
        }
    }

    let mut ctx = sc::ecs::schedule::Ctx::default();
    // Try to enqueue MM — cast_system should reject and push a HUD toast: code 1
    s.enqueue_cast(
        glam::vec3(0.0, 0.6, 0.0),
        glam::vec3(0.0, 0.0, 1.0),
        sc::SpellId::MagicMissile,
    );
    sc::ecs::schedule::cast_system(&mut s, &mut ctx);
    assert!(
        ctx.hud_toasts.contains(&1u8),
        "expected 'not enough mana' toast"
    );
}
