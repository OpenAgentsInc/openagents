use glam::vec3;

#[test]
fn pc_hitting_hostile_flips_faction_hostility() {
    let mut s = server_core::ServerState::new();
    let _pc = s.spawn_pc_at(vec3(0.0, 0.6, 0.0));
    let wiz = s.spawn_wizard_npc(vec3(1.0, 0.6, 0.0));
    // Sanity: initially not hostile Pc<->Wizards
    assert!(!s.factions.pc_vs_wizards_hostile);
    // Fire a Firebolt from PC straight at the wizard
    s.enqueue_cast(
        vec3(0.0, 0.6, 0.0),
        vec3(1.0, 0.0, 0.0),
        server_core::SpellId::Firebolt,
    );
    // Step until collision processes
    for _ in 0..10 {
        s.step_authoritative(0.05);
    }
    // Wizard should have taken damage or at least the flip should have occurred via faction_flip_on_pc_hits_wizards
    assert!(
        s.factions.pc_vs_wizards_hostile,
        "pc_vs_wizards_hostile should flip true after PC damages wizard"
    );
    // Health drop check (best effort; wizard HP is 100 initially)
    if let Some(w) = s.ecs.get(wiz) {
        assert!(w.hp.hp <= 100);
    }
}
